#include "router.h"

#include <QLoggingCategory>

namespace dettivo {

Q_LOGGING_CATEGORY(lcRouter, "dettivo.app.router")

namespace {

const QStringList kSettings = {
    QStringLiteral("general"), QStringLiteral("vocabulary"),   QStringLiteral("hotkeys"),  QStringLiteral("models"),
    QStringLiteral("polish"),    QStringLiteral("insertion"), QStringLiteral("meetings"),
    QStringLiteral("agents"),    QStringLiteral("diagnostics"),
};

}  // namespace

Router::Router(QObject *parent) : QObject(parent) {}

QStringList Router::settingsSections()
{
    return kSettings;
}

QStringList Router::routeNames()
{
    QStringList names = {QStringLiteral("onboarding"),      QStringLiteral("home"),
                         QStringLiteral("history"),         QStringLiteral("history.detail"),
                         QStringLiteral("meetings"),        QStringLiteral("meetings.live"),
                         QStringLiteral("meetings.detail"), QStringLiteral("settings")};
    for (const QString &section : kSettings)
        names.append(QStringLiteral("settings.") + section);
    names.append(QStringLiteral("agents"));
    return names;
}

bool Router::parse(const QString &name, QString *route, QString *sub, QString *error)
{
    const QString trimmed = name.trimmed();
    QString r = trimmed;
    QString s;
    if (trimmed == QStringLiteral("agents")) {
        r = QStringLiteral("settings");
        s = QStringLiteral("agents");
    } else if (const auto dot = trimmed.indexOf(QLatin1Char('.')); dot > 0) {
        r = trimmed.left(dot);
        s = trimmed.mid(dot + 1);
    }
    bool ok = false;
    if (r == QStringLiteral("home") || r == QStringLiteral("onboarding"))
        ok = s.isEmpty();
    else if (r == QStringLiteral("history"))
        ok = s.isEmpty() || s == QStringLiteral("detail");
    else if (r == QStringLiteral("meetings"))
        ok = s.isEmpty() || s == QStringLiteral("live") || s == QStringLiteral("detail");
    else if (r == QStringLiteral("settings"))
        ok = s.isEmpty() || kSettings.contains(s);
    if (!ok) {
        if (error != nullptr)
            *error = QStringLiteral("unknown route \"%1\"; the routes are %2")
                         .arg(trimmed, routeNames().join(QStringLiteral(", ")));
        return false;
    }
    if (route != nullptr)
        *route = r;
    if (sub != nullptr)
        *sub = s;
    return true;
}

QString Router::pageFor(const QString &route, const QString &sub)
{
    return sub.isEmpty() ? route : route + QLatin1Char('.') + sub;
}

QString Router::titleFor(const QString &route, const QString &sub)
{
    if (route == QStringLiteral("home"))
        return tr("Home");
    if (route == QStringLiteral("onboarding"))
        return tr("First run");
    if (route == QStringLiteral("history"))
        return sub.isEmpty() ? tr("History") : tr("Dictation");
    if (route == QStringLiteral("meetings")) {
        if (sub == QStringLiteral("live"))
            return tr("Meeting live");
        return sub.isEmpty() ? tr("Meetings") : tr("Meeting");
    }
    if (route == QStringLiteral("settings")) {
        if (sub.isEmpty())
            return tr("Settings");
        QString section = sub;
        section[0] = section[0].toUpper();
        return tr("Settings / %1").arg(section);
    }
    return route;
}

bool Router::isDetail() const
{
    return (m_route == QStringLiteral("history") && !m_sub.isEmpty())
        || (m_route == QStringLiteral("meetings") && !m_sub.isEmpty());
}

bool Router::open(const QString &name, const QString &arg)
{
    QString route, sub, error;
    if (!parse(name, &route, &sub, &error)) {
        qCWarning(lcRouter).noquote() << error;
        return false;
    }
    if (route == m_route && sub == m_sub && arg == m_arg)
        return true;
    m_stack.append({m_route, m_sub, m_arg});
    if (m_stack.size() > 32)
        m_stack.removeFirst();
    m_route = route;
    m_sub = sub;
    m_arg = arg;
    emit routeChanged();
    return true;
}

void Router::back()
{
    if (isDetail()) {
        // A detail returns to its list (docs/app.md): the list is the page
        // the detail belongs to, whatever was open before it. The entries
        // of the same route on top of the stack are dropped with it, so the
        // next Escape leaves the route instead of replaying it.
        m_sub.clear();
        m_arg.clear();
        while (!m_stack.isEmpty() && m_stack.last().route == m_route)
            m_stack.removeLast();
        emit routeChanged();
        return;
    }
    if (m_stack.isEmpty())
        return;
    const Entry previous = m_stack.takeLast();
    m_route = previous.route;
    m_sub = previous.sub;
    m_arg = previous.arg;
    emit routeChanged();
}

}  // namespace dettivo
