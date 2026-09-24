#include "config_binding.h"

#include <QJsonObject>
#include <QJsonValue>

namespace dettivo {

ConfigBinding::ConfigBinding(DaemonLink *link, QObject *parent) : QObject(parent), m_link(link) {}

QVariant ConfigBinding::value(const QString &key) const
{
    return m_entries.value(key).value;
}

QString ConfigBinding::text(const QString &key) const
{
    const QVariant v = value(key);
    if (!v.isValid())
        return QString();
    if (v.typeId() == QMetaType::QVariantList) {
        QStringList parts;
        for (const QVariant &item : v.toList())
            parts.append(item.toString());
        return parts.join(QStringLiteral(", "));
    }
    return v.toString();
}

QString ConfigBinding::source(const QString &key) const
{
    return m_entries.value(key).source;
}

void ConfigBinding::applyEntries(const QJsonArray &entries)
{
    for (const QJsonValue &entry : entries) {
        const QJsonObject o = entry.toObject();
        const QString key = o.value(QStringLiteral("key")).toString();
        if (key.isEmpty())
            continue;
        m_entries.insert(key, {o.value(QStringLiteral("value")).toVariant(),
                               o.value(QStringLiteral("source")).toString()});
    }
    ++m_revision;
    emit changed();
}

void ConfigBinding::refresh()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("config.get"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        applyEntries(result.value(QStringLiteral("entries")).toArray());
    });
}

void ConfigBinding::set(const QString &key, const QVariant &value)
{
    if (m_link == nullptr || !m_link->connected()) {
        emit setFailed(key, QStringLiteral("the daemon is not connected"));
        return;
    }
    const QJsonObject params{{QStringLiteral("key"), key},
                             {QStringLiteral("value"), QJsonValue::fromVariant(value)}};
    m_link->call(QStringLiteral("config.set"), params, [this, key](const QJsonObject &, const QJsonObject &error) {
        if (!error.isEmpty()) {
            emit setFailed(key, error.value(QStringLiteral("message")).toString());
            return;
        }
        refresh();
    });
}

}  // namespace dettivo
