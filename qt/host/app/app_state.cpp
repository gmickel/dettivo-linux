#include "app_state.h"
#include "daemon_paths.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSaveFile>

#include <toml++/toml.hpp>

#include <sstream>

namespace dettivo {

namespace {

// The tables this app owns; every other table belongs to the daemon.
const char *const kOwnTables[] = {"window", "app", "first_run"};

// The daemon's tables as the file holds them at this moment, as TOML
// text; empty when the file is absent or does not parse.
QString daemonTables(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return QString();
    toml::table doc;
    try {
        doc = toml::parse(file.readAll().toStdString());
    } catch (const toml::parse_error &) {
        return QString();
    }
    for (const char *own : kOwnTables)
        doc.erase(own);
    if (doc.empty())
        return QString();
    std::ostringstream rest;
    rest << doc;
    return QString::fromStdString(rest.str()).trimmed();
}

}  // namespace

QString AppState::path(const QProcessEnvironment &env)
{
    return paths::stateDir(env) + QStringLiteral("/state.toml");
}

AppState AppState::load(const QString &path, QString *warning)
{
    QFile file(path);
    if (!file.exists())
        return AppState();
    if (!file.open(QIODevice::ReadOnly)) {
        if (warning != nullptr)
            *warning = QStringLiteral("%1: cannot read, starting from the default state").arg(path);
        return AppState();
    }
    return fromToml(QString::fromUtf8(file.readAll()), warning);
}

AppState AppState::fromToml(const QString &text, QString *warning)
{
    AppState s;
    toml::table doc;
    try {
        doc = toml::parse(text.toStdString());
    } catch (const toml::parse_error &err) {
        if (warning != nullptr)
            *warning = QStringLiteral("state.toml does not parse (%1); starting from the default state")
                           .arg(QString::fromStdString(std::string(err.description())));
        return s;
    }
    auto readInt = [](const toml::table *table, const char *key, int &into) {
        if (table == nullptr)
            return;
        if (const auto v = (*table)[key].value<int64_t>())
            into = int(*v);
    };
    auto readBool = [](const toml::table *table, const char *key, bool &into) {
        if (table == nullptr)
            return;
        if (const auto v = (*table)[key].value<bool>())
            into = *v;
    };
    auto readString = [](const toml::table *table, const char *key, QString &into) {
        if (table == nullptr)
            return;
        if (const auto v = (*table)[key].value<std::string>())
            into = QString::fromStdString(*v);
    };
    const auto *window = doc["window"].as_table();
    readInt(window, "width", s.width);
    readInt(window, "height", s.height);
    readInt(window, "x", s.x);
    readInt(window, "y", s.y);
    readBool(window, "maximized", s.maximized);
    const auto *app = doc["app"].as_table();
    readString(app, "last_route", s.lastRoute);
    readString(app, "last_meeting_tab", s.lastMeetingTab);
    const auto *firstRun = doc["first_run"].as_table();
    readString(firstRun, "completed_at", s.firstRunCompletedAt);
    readString(firstRun, "step", s.firstRunStep);
    if (s.lastRoute.isEmpty())
        s.lastRoute = QStringLiteral("home");
    if (s.lastMeetingTab.isEmpty())
        s.lastMeetingTab = QStringLiteral("transcript");
    return s;
}

QString AppState::toToml() const
{
    QString out;
    out += QStringLiteral("# Dettivo runtime state; configuration lives in config.toml.\n");
    out += QStringLiteral("[window]\nwidth = %1\nheight = %2\nx = %3\ny = %4\nmaximized = %5\n\n")
               .arg(width)
               .arg(height)
               .arg(x)
               .arg(y)
               .arg(maximized ? QStringLiteral("true") : QStringLiteral("false"));
    out += QStringLiteral("[app]\nlast_route = \"%1\"\nlast_meeting_tab = \"%2\"\n\n").arg(lastRoute, lastMeetingTab);
    out += QStringLiteral("[first_run]\ncompleted_at = \"%1\"\nstep = \"%2\"\n").arg(firstRunCompletedAt, firstRunStep);
    return out;
}

bool AppState::save(const QString &path, QString *error) const
{
    const QString dir = QFileInfo(path).absolutePath();
    if (!QDir().mkpath(dir)) {
        if (error != nullptr)
            *error = QStringLiteral("cannot create %1").arg(dir);
        return false;
    }
    // The daemon may have written its tables since this state was loaded
    // (an acknowledgement, a start count): they are read now, so the
    // save never puts a load-time snapshot back.
    QString out = toToml();
    const QString rest = daemonTables(path);
    if (!rest.isEmpty())
        out += QStringLiteral("\n") + rest + QStringLiteral("\n");
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        if (error != nullptr)
            *error = QStringLiteral("%1: %2").arg(path, file.errorString());
        return false;
    }
    file.write(out.toUtf8());
    if (!file.commit()) {
        if (error != nullptr)
            *error = QStringLiteral("%1: %2").arg(path, file.errorString());
        return false;
    }
    return true;
}

}  // namespace dettivo
