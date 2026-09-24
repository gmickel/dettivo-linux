#include "osd_settings.h"
#include "daemon_paths.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>

#include <toml++/toml.hpp>

namespace dettivo {

namespace {

void note(QString *warning, const QString &line)
{
    if (warning == nullptr)
        return;
    if (!warning->isEmpty())
        warning->append(QLatin1Char('\n'));
    warning->append(line);
}

}  // namespace

QStringList OsdSettings::positions()
{
    return {QStringLiteral("top"),      QStringLiteral("bottom"),      QStringLiteral("top_left"),
            QStringLiteral("top_right"), QStringLiteral("bottom_left"), QStringLiteral("bottom_right")};
}

QStringList OsdSettings::hosts()
{
    return {QStringLiteral("auto"), QStringLiteral("layer_shell"), QStringLiteral("window")};
}

QString OsdSettings::configPath(const QProcessEnvironment &env)
{
    return paths::configFile(env);
}

QString OsdSettings::socketDir(const QProcessEnvironment &env)
{
    return paths::socketDir(env);
}

QString OsdSettings::daemonSocket(const QProcessEnvironment &env)
{
    return paths::daemonSocket(env);
}

OsdSettings OsdSettings::load(const QString &path, QString *warning)
{
    QFile file(path);
    if (!file.exists())
        return OsdSettings();
    if (!file.open(QIODevice::ReadOnly)) {
        note(warning, QStringLiteral("%1: cannot read, using the default [osd] settings").arg(path));
        return OsdSettings();
    }
    return fromToml(QString::fromUtf8(file.readAll()), warning);
}

OsdSettings OsdSettings::fromToml(const QString &text, QString *warning)
{
    OsdSettings s;
    toml::table doc;
    try {
        doc = toml::parse(text.toStdString());
    } catch (const toml::parse_error &err) {
        note(warning, QStringLiteral("config.toml does not parse (%1); using the default [osd] settings")
                          .arg(QString::fromStdString(std::string(err.description()))));
        return s;
    }
    const auto *osd = doc["osd"].as_table();
    if (osd == nullptr)
        return s;

    auto readBool = [&](const char *key, bool &into) {
        const auto node = (*osd)[key];
        if (!node)
            return;
        if (const auto v = node.value<bool>())
            into = *v;
        else
            note(warning, QStringLiteral("osd.%1: expected true or false").arg(QLatin1String(key)));
    };
    auto readInt = [&](const char *key, int &into, int minimum) {
        const auto node = (*osd)[key];
        if (!node)
            return;
        const auto v = node.value<int64_t>();
        if (v && *v >= minimum && *v <= 600000)
            into = int(*v);
        else
            note(warning, QStringLiteral("osd.%1: expected an integer of at least %2").arg(QLatin1String(key)).arg(minimum));
    };
    auto readChoice = [&](const char *key, QString &into, const QStringList &allowed) {
        const auto node = (*osd)[key];
        if (!node)
            return;
        const auto v = node.value<std::string>();
        const QString chosen = v ? QString::fromStdString(*v) : QString();
        if (v && allowed.contains(chosen))
            into = chosen;
        else
            note(warning, QStringLiteral("osd.%1: expected one of %2").arg(QLatin1String(key), allowed.join(QStringLiteral(", "))));
    };
    auto readString = [&](const char *key, QString &into) {
        const auto node = (*osd)[key];
        if (!node)
            return;
        if (const auto v = node.value<std::string>())
            into = QString::fromStdString(*v);
        else
            note(warning, QStringLiteral("osd.%1: expected a string").arg(QLatin1String(key)));
    };

    readBool("enabled", s.enabled);
    readChoice("host", s.host, hosts());
    readChoice("position", s.position, positions());
    readInt("margin", s.margin, 0);
    readString("monitor", s.monitor);
    readInt("hide_after_ms", s.hideAfterMs, 0);
    readInt("error_hide_after_ms", s.errorHideAfterMs, 0);
    readBool("show_level", s.showLevel);
    readChoice("motion", s.motion, {QStringLiteral("full"), QStringLiteral("reduced")});
    return s;
}

}  // namespace dettivo
