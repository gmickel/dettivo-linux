#include "hyprland.h"

#include <QFileInfo>
#include <QJsonDocument>
#include <QLocalSocket>

#include <cmath>

namespace dettivo {

Hyprland::Hyprland(const QProcessEnvironment &env)
{
    const QString signature = env.value(QStringLiteral("HYPRLAND_INSTANCE_SIGNATURE"));
    const QString runtime = env.value(QStringLiteral("XDG_RUNTIME_DIR"));
    if (signature.isEmpty() || runtime.isEmpty())
        return;
    const QString path = runtime + QStringLiteral("/hypr/") + signature + QStringLiteral("/.socket.sock");
    if (QFileInfo::exists(path))
        m_socket = path;
}

QByteArray Hyprland::request(const QByteArray &command) const
{
    if (m_socket.isEmpty())
        return {};
    QLocalSocket socket;
    socket.connectToServer(m_socket);
    if (!socket.waitForConnected(200))
        return {};
    socket.write(command);
    socket.flush();
    QByteArray out;
    while (socket.waitForReadyRead(200))
        out.append(socket.readAll());
    out.append(socket.readAll());
    return out;
}

std::optional<HyprWindow> Hyprland::activeWindow() const
{
    const QJsonDocument doc = QJsonDocument::fromJson(request("j/activewindow"));
    if (!doc.isObject())
        return std::nullopt;
    return parseActiveWindow(doc.object());
}

QList<HyprMonitor> Hyprland::monitors() const
{
    const QJsonDocument doc = QJsonDocument::fromJson(request("j/monitors"));
    if (!doc.isArray())
        return {};
    return parseMonitors(doc.array());
}

std::optional<QString> Hyprland::focusedMonitorName() const
{
    const QList<HyprMonitor> list = monitors();
    for (const HyprMonitor &m : list) {
        if (m.focused)
            return m.name;
    }
    return std::nullopt;
}

std::optional<HyprWindow> Hyprland::parseActiveWindow(const QJsonObject &object)
{
    const QJsonArray at = object.value(QStringLiteral("at")).toArray();
    const QJsonArray size = object.value(QStringLiteral("size")).toArray();
    if (at.size() != 2 || size.size() != 2)
        return std::nullopt;
    HyprWindow w;
    w.rect = {at.at(0).toInt(), at.at(1).toInt(), size.at(0).toInt(), size.at(1).toInt()};
    w.monitor = object.value(QStringLiteral("monitor")).toInt(-1);
    w.appClass = object.value(QStringLiteral("class")).toString();
    if (w.rect.width <= 0 || w.rect.height <= 0)
        return std::nullopt;
    return w;
}

QList<HyprMonitor> Hyprland::parseMonitors(const QJsonArray &array)
{
    QList<HyprMonitor> out;
    for (const QJsonValue &value : array) {
        const QJsonObject o = value.toObject();
        HyprMonitor m;
        m.id = o.value(QStringLiteral("id")).toInt(-1);
        m.name = o.value(QStringLiteral("name")).toString();
        const double scale = qMax(o.value(QStringLiteral("scale")).toDouble(1.0), 0.1);
        m.rect = {o.value(QStringLiteral("x")).toInt(), o.value(QStringLiteral("y")).toInt(),
                  int(std::lround(o.value(QStringLiteral("width")).toDouble() / scale)),
                  int(std::lround(o.value(QStringLiteral("height")).toDouble() / scale))};
        m.focused = o.value(QStringLiteral("focused")).toBool(false);
        if (!m.name.isEmpty())
            out.append(m);
    }
    return out;
}

bool Hyprland::shouldFlip(bool anchoredTop, const HyprRect &monitor, int band, const HyprRect &window)
{
    const int top = monitor.y;
    const int bottom = monitor.y + monitor.height;
    const int windowTop = window.y;
    const int windowBottom = window.y + window.height;
    const bool horizontalOverlap = window.x < monitor.x + monitor.width && window.x + window.width > monitor.x;
    if (!horizontalOverlap)
        return false;
    const bool touchesTop = windowTop < top + band && windowBottom > top;
    const bool touchesBottom = windowBottom > bottom - band && windowTop < bottom;
    return anchoredTop ? (touchesTop && !touchesBottom) : (touchesBottom && !touchesTop);
}

}  // namespace dettivo
