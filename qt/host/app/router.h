// The app's routes (FR-U2): one name per screen, the settings sub-routes,
// a history stack for Escape, and the accessible title the drives assert.
// `DETTIVO_E2E_OPEN`, `dettivo app open` and the sidebar all go through
// `open`, so a name that is not here is refused in one place.
#pragma once

#include <QObject>
#include <QString>
#include <QStringList>

#include <QList>

namespace dettivo {

class Router : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString route READ route NOTIFY routeChanged)
    Q_PROPERTY(QString sub READ sub NOTIFY routeChanged)
    Q_PROPERTY(QString arg READ arg NOTIFY routeChanged)
    Q_PROPERTY(QString page READ page NOTIFY routeChanged)
    Q_PROPERTY(QString title READ title NOTIFY routeChanged)
    Q_PROPERTY(bool detail READ isDetail NOTIFY routeChanged)
    Q_PROPERTY(bool canGoBack READ canGoBack NOTIFY routeChanged)
    Q_PROPERTY(QStringList settingsSections READ settingsSections CONSTANT)

public:
    explicit Router(QObject *parent = nullptr);

    /// Every name `open` accepts, in the order docs/app.md lists them.
    static QStringList routeNames();
    /// The settings sub-routes, in tab order.
    static QStringList settingsSections();
    /// Splits `name` into the route and its sub-route; `error` names the
    /// accepted routes when the name is not one of them.
    static bool parse(const QString &name, QString *route, QString *sub, QString *error);
    /// The accessible title of a page (`Settings / Hotkeys`).
    static QString titleFor(const QString &route, const QString &sub);
    /// The page name (`history.detail`) of a route and sub-route.
    static QString pageFor(const QString &route, const QString &sub);

    QString route() const { return m_route; }
    QString sub() const { return m_sub; }
    QString arg() const { return m_arg; }
    QString page() const { return pageFor(m_route, m_sub); }
    QString title() const { return titleFor(m_route, m_sub); }
    bool isDetail() const;
    bool canGoBack() const { return isDetail() || !m_stack.isEmpty(); }

    /// Opens a route by name (`history`, `settings.hotkeys`, `agents`);
    /// false with a warning for an unknown name. An `arg` names the item
    /// a detail route shows.
    Q_INVOKABLE bool open(const QString &name, const QString &arg = QString());
    /// A detail route returns to its list; any other page returns to the
    /// previous page.
    Q_INVOKABLE void back();

signals:
    void routeChanged();

private:
    struct Entry {
        QString route, sub, arg;
    };
    QString m_route = QStringLiteral("home");
    QString m_sub;
    QString m_arg;
    QList<Entry> m_stack;
};

}  // namespace dettivo
