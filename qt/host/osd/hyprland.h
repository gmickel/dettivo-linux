// Hyprland IPC for the pill: the focused window's geometry (so the pill
// never covers its caret region) and the monitors (so `monitor =
// "focused"` follows the focused output). Requests go over Hyprland's own
// socket, the same one hyprctl uses; without Hyprland every call answers
// "nothing known" and the pill keeps its configured edge.
#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QProcessEnvironment>
#include <QString>

#include <optional>

namespace dettivo {

struct HyprRect {
    int x = 0;
    int y = 0;
    int width = 0;
    int height = 0;
};

struct HyprWindow {
    HyprRect rect;
    int monitor = -1;
    QString appClass;
};

struct HyprMonitor {
    int id = -1;
    QString name;
    HyprRect rect;  ///< Logical (scaled) size at the layout position.
    bool focused = false;
};

class Hyprland {
public:
    explicit Hyprland(const QProcessEnvironment &env = QProcessEnvironment::systemEnvironment());

    /// True when the session variables name a running Hyprland.
    bool available() const { return !m_socket.isEmpty(); }
    /// The request socket (`$XDG_RUNTIME_DIR/hypr/<signature>/.socket.sock`).
    QString socketPath() const { return m_socket; }

    std::optional<HyprWindow> activeWindow() const;
    QList<HyprMonitor> monitors() const;
    /// The focused monitor's name, when Hyprland answers.
    std::optional<QString> focusedMonitorName() const;

    /// Whether the pill should leave its anchored edge: the focused window
    /// reaches into the pill's band on that edge but not into the band on
    /// the opposite edge, so the opposite edge is free.
    static bool shouldFlip(bool anchoredTop, const HyprRect &monitor, int band, const HyprRect &window);

    static std::optional<HyprWindow> parseActiveWindow(const QJsonObject &object);
    static QList<HyprMonitor> parseMonitors(const QJsonArray &array);

private:
    QByteArray request(const QByteArray &command) const;

    QString m_socket;
};

}  // namespace dettivo
