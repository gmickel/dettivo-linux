#include "osd_host.h"
#include "osd_model.h"

#include <QGuiApplication>
#include <QLoggingCategory>
#include <QTimer>

#include <cmath>
#include <QQuickItem>
#include <QQuickWindow>
#include <QScreen>

#ifdef DETTIVO_HAVE_LAYER_SHELL
#include <LayerShellQt/Window>
#endif

namespace dettivo {

Q_LOGGING_CATEGORY(lcOsdHost, "dettivo.osd.host")

OsdHost::OsdHost(OsdSettings settings, QObject *parent)
    : QObject(parent), m_settings(std::move(settings)), m_effectivePosition(m_settings.position)
{
}

OsdHost::Kind OsdHost::decide(const QString &hostSetting, const QString &platform, bool layerShellBuilt,
                              bool layerShellOffered, QString *notice)
{
    const bool wayland = platform == QStringLiteral("wayland");
    const bool layerShell = wayland && layerShellBuilt && layerShellOffered;
    if (hostSetting == QStringLiteral("window"))
        return Kind::Window;
    if (hostSetting == QStringLiteral("layer_shell")) {
        if (layerShell)
            return Kind::LayerShell;
        if (notice != nullptr) {
            *notice = !wayland ? QStringLiteral("[osd] host = \"layer_shell\" but the session is not Wayland (%1); set host = \"auto\" or \"window\"").arg(platform)
                : !layerShellBuilt ? QStringLiteral("[osd] host = \"layer_shell\" but dettivo-osd was built without layer-shell-qt; set host = \"auto\" or \"window\"")
                                   : QStringLiteral("[osd] host = \"layer_shell\" but the compositor offers no zwlr_layer_shell_v1; set host = \"auto\" or \"window\"");
        }
        return Kind::Disabled;
    }
    return layerShell ? Kind::LayerShell : Kind::Window;
}

QString OsdHost::kindName(Kind kind)
{
    switch (kind) {
    case Kind::LayerShell:
        return QStringLiteral("layer_shell");
    case Kind::Window:
        return QStringLiteral("window");
    case Kind::Disabled:
        break;
    }
    return QStringLiteral("disabled");
}

void OsdHost::attach(Kind kind, QQuickWindow *window, QQuickItem *pill, OsdModel *model)
{
    m_kind = kind;
    m_window = window;
    m_pill = pill;
    m_model = model;
    if (kind == Kind::LayerShell)
        applyLayerShell();
    connect(model, &OsdModel::stateChanged, this, [this]() {
        if (m_model->visible()) {
            m_hidePending = false;
            place();
            showWindow();
        } else {
            m_hidePending = true;
            // The pill fades out first; the window goes when the fade ends
            // (immediately under reduced motion, where opacity is a cut).
            if (m_pill == nullptr || qFuzzyIsNull(m_pill->opacity()))
                hideWindow();
        }
    });
    if (pill != nullptr) {
        connect(pill, &QQuickItem::opacityChanged, this, [this]() {
            if (m_hidePending && qFuzzyIsNull(m_pill->opacity()))
                hideWindow();
        });
        connect(pill, &QQuickItem::implicitWidthChanged, this, &OsdHost::syncSize);
        connect(pill, &QQuickItem::implicitHeightChanged, this, &OsdHost::syncSize);
    }
    m_model->setHostFacts(kindName(kind), m_effectivePosition, m_monitorName);
}

QScreen *OsdHost::chooseScreen() const
{
    const QList<QScreen *> screens = QGuiApplication::screens();
    QString wanted = m_settings.monitor;
    if (wanted == QStringLiteral("focused")) {
        const auto focused = m_hyprland.focusedMonitorName();
        wanted = focused.value_or(QString());
    }
    if (!wanted.isEmpty()) {
        for (QScreen *s : screens) {
            if (s->name() == wanted)
                return s;
        }
        if (m_settings.monitor != QStringLiteral("focused"))
            qCInfo(lcOsdHost) << "output" << wanted << "is not connected; using the primary";
    }
    return QGuiApplication::primaryScreen();
}

void OsdHost::place()
{
    QScreen *screen = chooseScreen();
    m_monitorName = screen != nullptr ? screen->name() : QString();
    m_effectivePosition = m_settings.position;

    // Caret avoidance on Hyprland: when the focused window reaches into
    // the pill's band on the configured edge and leaves the opposite band
    // free, the pill takes the opposite edge for this show.
    if (m_hyprland.available() && screen != nullptr) {
        const auto window = m_hyprland.activeWindow();
        const QList<HyprMonitor> monitors = m_hyprland.monitors();
        const auto monitor = std::find_if(monitors.begin(), monitors.end(),
                                          [&](const HyprMonitor &m) { return m.name == m_monitorName; });
        if (window.has_value() && monitor != monitors.end()) {
            const int band = int(m_window != nullptr ? m_window->height() : 36) + m_settings.margin;
            if (Hyprland::shouldFlip(m_settings.anchoredTop(), monitor->rect, band, window->rect)) {
                m_effectivePosition = m_settings.anchoredTop()
                    ? QString(m_settings.position).replace(QStringLiteral("top"), QStringLiteral("bottom"))
                    : QString(m_settings.position).replace(QStringLiteral("bottom"), QStringLiteral("top"));
                qCDebug(lcOsdHost) << "focused window" << window->appClass << "reaches the" << m_settings.position
                                   << "band; pill moves to" << m_effectivePosition;
            }
        }
    }
    if (m_window != nullptr && screen != nullptr && m_window->screen() != screen)
        m_window->setScreen(screen);
    if (m_kind == Kind::LayerShell)
        applyLayerShell();
    else
        applyWindowPosition();
    if (m_model != nullptr)
        m_model->setHostFacts(kindName(m_kind), m_effectivePosition, m_monitorName);
}

void OsdHost::applyLayerShell()
{
#ifdef DETTIVO_HAVE_LAYER_SHELL
    if (m_window == nullptr)
        return;
    auto *layer = LayerShellQt::Window::get(m_window);
    layer->setLayer(LayerShellQt::Window::LayerOverlay);
    layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
    layer->setExclusiveZone(0);
    layer->setScope(QStringLiteral("dettivo-osd"));
    layer->setActivateOnShow(false);
    const QString p = m_effectivePosition;
    LayerShellQt::Window::Anchors anchors = p.startsWith(QStringLiteral("top")) ? LayerShellQt::Window::AnchorTop
                                                                                 : LayerShellQt::Window::AnchorBottom;
    if (p.endsWith(QStringLiteral("_left")))
        anchors |= LayerShellQt::Window::AnchorLeft;
    else if (p.endsWith(QStringLiteral("_right")))
        anchors |= LayerShellQt::Window::AnchorRight;
    layer->setAnchors(anchors);
    const int m = m_settings.margin;
    layer->setMargins(QMargins(anchors & LayerShellQt::Window::AnchorLeft ? m : 0,
                               anchors & LayerShellQt::Window::AnchorTop ? m : 0,
                               anchors & LayerShellQt::Window::AnchorRight ? m : 0,
                               anchors & LayerShellQt::Window::AnchorBottom ? m : 0));
    layer->setDesiredSize(m_window->size());
    if (m_window->screen() != nullptr)
        layer->setScreen(m_window->screen());
#endif
}

void OsdHost::applyWindowPosition()
{
    if (m_window == nullptr)
        return;
    QScreen *screen = m_window->screen();
    if (screen == nullptr)
        return;
    const QRect area = screen->availableGeometry();
    const int m = m_settings.margin;
    const QString p = m_effectivePosition;
    int x = area.x() + (area.width() - m_window->width()) / 2;
    if (p.endsWith(QStringLiteral("_left")))
        x = area.x() + m;
    else if (p.endsWith(QStringLiteral("_right")))
        x = area.x() + area.width() - m_window->width() - m;
    const int y = p.startsWith(QStringLiteral("top")) ? area.y() + m : area.y() + area.height() - m_window->height() - m;
    // On Wayland without layer shell the compositor places the window; the
    // request is still made so X11 and Xvfb honour it.
    m_window->setPosition(x, y);
}

void OsdHost::syncSize()
{
    if (m_window == nullptr || m_pill == nullptr)
        return;
    const QSize size(std::max(1, int(std::ceil(m_pill->implicitWidth()))),
                     std::max(1, int(std::ceil(m_pill->implicitHeight()))));
    if (m_window->size() != size)
        m_window->resize(size);
    if (m_kind == Kind::LayerShell)
        applyLayerShell();
    else
        applyWindowPosition();
}

void OsdHost::showWindow()
{
    if (m_window == nullptr)
        return;
    syncSize();
    if (!m_window->isVisible())
        m_window->show();
    // The positioner settles on the first frame after the map; the size is
    // re-applied once that frame has run.
    QTimer::singleShot(0, this, &OsdHost::syncSize);
}

void OsdHost::hideWindow()
{
    m_hidePending = false;
    if (m_window != nullptr && m_window->isVisible())
        m_window->hide();
}

}  // namespace dettivo
