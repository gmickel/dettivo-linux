// Where the pill's window goes (fn-12 R4): a layer-shell overlay anchored
// per [osd] position on wlroots compositors and KDE, a frameless
// always-on-top tool window elsewhere, and a notice when neither can
// work. Placement is re-evaluated every time the pill shows: the monitor
// setting, and on Hyprland the focused window's geometry so the pill
// leaves an edge the caret region reaches into.
#pragma once

#include "hyprland.h"
#include "osd_settings.h"

#include <QObject>
#include <QString>

class QQuickWindow;
class QQuickItem;
class QScreen;

namespace dettivo {

class OsdModel;

class OsdHost : public QObject {
    Q_OBJECT

public:
    enum class Kind { LayerShell, Window, Disabled };

    OsdHost(OsdSettings settings, QObject *parent = nullptr);

    /// Picks the host from the setting, the platform and the probe;
    /// `notice` explains a Disabled answer.
    static Kind decide(const QString &hostSetting, const QString &platform, bool layerShellBuilt,
                       bool layerShellOffered, QString *notice);
    static QString kindName(Kind kind);

    /// Binds the window: layer-shell configuration or fallback flags, and
    /// show/hide from the model with the pill's exit fade honoured.
    void attach(Kind kind, QQuickWindow *window, QQuickItem *pill, OsdModel *model);

    Kind kind() const { return m_kind; }
    /// The edge the pill sits on right now (`top` or `bottom` variants).
    QString effectivePosition() const { return m_effectivePosition; }
    /// The output the pill sits on.
    QString monitorName() const { return m_monitorName; }

    /// Chooses the screen and the edge for the next show.
    void place();
    /// Sizes the window to the pill; the platform may have set another
    /// size on map, so this is re-applied after every show and layout.
    void syncSize();

private:
    QScreen *chooseScreen() const;
    void applyLayerShell();
    void applyWindowPosition();
    void showWindow();
    void hideWindow();

    OsdSettings m_settings;
    Hyprland m_hyprland;
    Kind m_kind = Kind::Window;
    QQuickWindow *m_window = nullptr;
    QQuickItem *m_pill = nullptr;
    OsdModel *m_model = nullptr;
    QString m_effectivePosition;
    QString m_monitorName;
    bool m_hidePending = false;
};

}  // namespace dettivo
