#include "settings_model.h"

#include "config_binding.h"
#include "settings_keys.h"

#include <QClipboard>
#include <QDesktopServices>
#include <QGuiApplication>
#include <QJsonObject>
#include <QJsonValue>
#include <QProcessEnvironment>
#include <QUrl>

namespace dettivo {

SettingsModel::SettingsModel(DaemonLink *link, ConfigBinding *config, QObject *parent)
    : QObject(parent), m_link(link), m_config(config)
{
    if (m_config != nullptr) {
        connect(m_config, &ConfigBinding::changed, this, [this]() {
            ++m_revision;
            emit changed();
        });
        connect(m_config, &ConfigBinding::setFailed, this, [this](const QString &key, const QString &message) {
            m_errors.insert(key, message);
            emit errorChanged(key);
        });
    }
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::connectedChanged, this, &SettingsModel::onConnected);
        connect(m_link, &DaemonLink::notification, this, &SettingsModel::handleNotification);
    }
}

void SettingsModel::start()
{
    setActive(true);
}

void SettingsModel::setActive(bool active)
{
    if (m_active == active)
        return;
    m_active = active;
    if (m_link != nullptr && m_link->connected())
        onConnected(true);
}

void SettingsModel::onConnected(bool connected)
{
    if (!connected || !m_active)
        return;
    readRegistry();
    readPath();
}

void SettingsModel::readRegistry()
{
    m_link->call(QStringLiteral("config.keys"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyRegistry(result.value(QStringLiteral("keys")).toArray());
    });
}

void SettingsModel::readPath()
{
    m_link->call(QStringLiteral("config.path"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QString path = result.value(QStringLiteral("config")).toString();
        if (path == m_configPath)
            return;
        m_configPath = path;
        emit pathChanged();
    });
}

void SettingsModel::applyRegistry(const QJsonArray &keys)
{
    m_registry.clear();
    for (const QJsonValue &v : keys) {
        const QJsonObject k = v.toObject();
        m_registry.insert(k.value(QStringLiteral("key")).toString(),
                          {k.value(QStringLiteral("section")).toString(), k.value(QStringLiteral("kind")).toString(),
                           k.value(QStringLiteral("doc")).toString(), k.value(QStringLiteral("default")).toVariant()});
    }
    emit registryChanged();
}

void SettingsModel::applySample()
{
    m_configPath = QStringLiteral("~/.config/dettivo/config.toml");
    emit pathChanged();
}

QVariant SettingsModel::value(const QString &key) const
{
    return m_config != nullptr ? m_config->value(key) : QVariant();
}

QString SettingsModel::text(const QString &key) const
{
    const QVariant v = value(key);
    if (v.typeId() == QMetaType::QVariantMap) {
        QStringList parts;
        const QVariantMap map = v.toMap();
        for (auto it = map.cbegin(); it != map.cend(); ++it)
            parts.append(it.key() + QLatin1Char('=') + it.value().toString());
        return parts.join(QStringLiteral(", "));
    }
    return m_config != nullptr ? m_config->text(key) : QString();
}

QString SettingsModel::source(const QString &key) const
{
    return m_config != nullptr ? m_config->source(key) : QString();
}

QString SettingsModel::lockedBy(const QString &key) const
{
    return source(key) == QStringLiteral("environment") ? settings::environmentVariable(key) : QString();
}

QString SettingsModel::doc(const QString &key) const
{
    return m_registry.value(key).doc;
}

QString SettingsModel::kind(const QString &key) const
{
    return m_registry.value(key).kind;
}

QVariant SettingsModel::defaultValue(const QString &key) const
{
    return m_registry.value(key).defaultValue;
}

QString SettingsModel::defaultText(const QString &key) const
{
    if (!m_registry.contains(key))
        return {};
    return tomlValue(m_registry.value(key).defaultValue);
}

QString SettingsModel::error(const QString &key) const
{
    return m_errors.value(key);
}

void SettingsModel::set(const QString &key, const QVariant &value)
{
    if (key == QStringLiteral("dictation.vocabulary") || key == QStringLiteral("polish.transforms")) {
        writeList(key, {{QStringLiteral("key"), key}, {QStringLiteral("value"), QJsonValue::fromVariant(value)}},
                  QStringLiteral("config.set"));
        return;
    }
    if (m_errors.remove(key) > 0)
        emit errorChanged(key);
    if (m_config != nullptr)
        m_config->set(key, value);
}

void SettingsModel::unset(const QString &key)
{
    if (key == QStringLiteral("dictation.vocabulary") || key == QStringLiteral("polish.transforms")) {
        writeList(key, {{QStringLiteral("key"), key}}, QStringLiteral("config.unset"));
        return;
    }
    if (m_errors.remove(key) > 0)
        emit errorChanged(key);
    if (m_link == nullptr || !m_link->connected()) {
        m_errors.insert(key, tr("The daemon is not connected."));
        emit errorChanged(key);
        return;
    }
    m_link->call(QStringLiteral("config.unset"), QJsonObject{{QStringLiteral("key"), key}},
                 [this, key](const QJsonObject &, const QJsonObject &error) {
                     if (!error.isEmpty()) {
                         m_errors.insert(key, error.value(QStringLiteral("message")).toString());
                         emit errorChanged(key);
                         return;
                     }
                     if (m_config != nullptr)
                         m_config->refresh();
                 });
}

// A list editor builds its next value from the confirmed snapshot. Keep its
// controls locked through this write's own refresh, including reset and no-ops.
void SettingsModel::writeList(const QString &key, const QJsonObject &params, const QString &method)
{
    if (pending(key))
        return;
    if (m_errors.remove(key) > 0)
        emit errorChanged(key);
    if (m_link == nullptr || !m_link->connected()) {
        finishList(key, {{QStringLiteral("message"), tr("The daemon is not connected.")}});
        return;
    }
    m_pending.append(key);
    ++m_revision;
    emit changed();
    m_link->call(method, params, [this, key](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty()) {
            finishList(key, error);
            return;
        }
        // set/unset return the authoritative entry. Preserve that acknowledgement
        // even if the following full refresh loses its connection.
        if (m_config != nullptr && result.value(QStringLiteral("key")).toString() == key)
            m_config->applyEntries({result});
        m_link->call(QStringLiteral("config.get"), {}, [this, key](const QJsonObject &snapshot, const QJsonObject &readError) {
            if (readError.isEmpty() && m_config != nullptr)
                m_config->applyEntries(snapshot.value(QStringLiteral("entries")).toArray());
            finishList(key, readError);
        });
    });
}

void SettingsModel::finishList(const QString &key, const QJsonObject &error)
{
    if (!error.isEmpty()) {
        m_errors.insert(key, error.value(QStringLiteral("message")).toString());
        emit errorChanged(key);
    }
    m_pending.removeAll(key);
    ++m_revision;
    emit changed();
}

QStringList SettingsModel::keysFor(const QString &section) const
{
    return settings::keysFor(section);
}

QString SettingsModel::tomlValue(const QVariant &value)
{
    switch (value.typeId()) {
    case QMetaType::Bool:
        return value.toBool() ? QStringLiteral("true") : QStringLiteral("false");
    case QMetaType::Int:
    case QMetaType::LongLong:
    case QMetaType::UInt:
    case QMetaType::ULongLong:
        return QString::number(value.toLongLong());
    case QMetaType::Double:
        return QString::number(value.toDouble());
    case QMetaType::QVariantList: {
        QStringList parts;
        for (const QVariant &item : value.toList())
            parts.append(tomlValue(item));
        return QLatin1Char('[') + parts.join(QStringLiteral(", ")) + QLatin1Char(']');
    }
    case QMetaType::QVariantMap: {
        QStringList parts;
        const QVariantMap map = value.toMap();
        for (auto it = map.cbegin(); it != map.cend(); ++it)
            parts.append(QLatin1Char('"') + it.key() + QStringLiteral("\" = ") + tomlValue(it.value()));
        return parts.isEmpty() ? QStringLiteral("{}") : QStringLiteral("{ ") + parts.join(QStringLiteral(", ")) + QStringLiteral(" }");
    }
    default: {
        QString s = value.toString();
        s.replace(QLatin1Char('\\'), QStringLiteral("\\\\")).replace(QLatin1Char('"'), QStringLiteral("\\\""));
        return QLatin1Char('"') + s + QLatin1Char('"');
    }
    }
}

QString SettingsModel::writesBlock(const QString &section) const
{
    // One line per table, the table name leading and the assignments after
    // it in row order, wrapped under the first assignment when a table has
    // more than a line holds, so the block reads like the file it describes.
    constexpr int kWidth = 92;
    constexpr int kColumn = 20;
    QStringList tables;
    QHash<QString, QStringList> entries;
    for (const QString &key : settings::keysFor(section)) {
        const QString table = settings::tableOf(key);
        if (!tables.contains(table))
            tables.append(table);
        const QVariant v = value(key);
        const QString rendered = v.isValid() ? tomlValue(v) : QStringLiteral("…");
        entries[table].append(settings::leafOf(key) + QStringLiteral(" = ") + rendered);
    }
    QStringList lines;
    for (const QString &table : tables) {
        QString line = (QLatin1Char('[') + table + QLatin1Char(']')).leftJustified(kColumn, QLatin1Char(' '));
        bool first = true;
        for (const QString &entry : entries.value(table)) {
            if (!first && line.length() + entry.length() + 4 > kWidth) {
                lines.append(line);
                line = QString(kColumn, QLatin1Char(' '));
            } else if (!first) {
                line += QStringLiteral("    ");
            }
            line += entry;
            first = false;
        }
        lines.append(line);
    }
    return lines.join(QLatin1Char('\n'));
}

void SettingsModel::openConfig()
{
    QString path = m_configPath;
    if (path.isEmpty())
        return;
    if (path.startsWith(QStringLiteral("~/")))
        path = QProcessEnvironment::systemEnvironment().value(QStringLiteral("HOME")) + path.mid(1);
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

void SettingsModel::copyText(const QString &text) const
{
    if (QClipboard *clipboard = QGuiApplication::clipboard())
        clipboard->setText(text);
}

void SettingsModel::setNotice(const QString &notice)
{
    if (m_notice == notice)
        return;
    m_notice = notice;
    emit noticeChanged();
}

void SettingsModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic != QStringLiteral("config.changed"))
        return;
    QStringList keys;
    for (const QJsonValue &k : payload.value(QStringLiteral("keys")).toArray())
        keys.append(k.toString());
    if (m_config != nullptr)
        m_config->refresh();
    if (payload.value(QStringLiteral("ok")).toBool(true))
        setNotice(keys.isEmpty() ? QString() : tr("The file changed on disk; %n value(s) refreshed.", nullptr, int(keys.size())));
    else
        setNotice(tr("The file changed on disk and does not parse; the daemon keeps the last valid settings."));
    emit fileChanged(keys);
}

}  // namespace dettivo
