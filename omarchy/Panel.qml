import QtQuick
import Quickshell
import Quickshell.Io

// The pill host: the manifest's `panel` kind, kept loaded for the life of
// the shell. While `[omarchy] osd = "panel"` and the shared module is
// there to draw with, it holds the session-bus name
// `dev.dettivo.OmarchyPanel` through `dettivo osd host-panel`, so
// `dettivo-osd` steps aside and one pill shows: the module's Osd on a
// layer-shell window at the `[osd]` position. The window shows only once
// the claim has answered `claimed`, and the claim ends when the pill
// cannot load, so the standalone pill is never suppressed for nothing.
// With `service` or `off` the name stays free and this window never shows.
Item {
    id: root

    readonly property bool hosting: DettivoState.osdMode === "panel" && DettivoState.moduleAvailable
    // What `dettivo osd host-panel` answered: true while it holds the name.
    property bool claimed: false
    readonly property bool atTop: DettivoState.osdPosition.indexOf("top") === 0
    readonly property bool atLeft: DettivoState.osdPosition.indexOf("_left") > 0
    readonly property bool atRight: DettivoState.osdPosition.indexOf("_right") > 0
    readonly property alias claiming: claim.running
    readonly property alias pillWindow: window

    // Nothing to draw here: the shell keeps the item mounted for the
    // process and the window.
    width: 0
    height: 0

    Process {
        id: claim
        command: [DettivoState.binary, "--json", "osd", "host-panel"]
        running: root.hosting && DettivoState.binaryAvailable && pill.status !== Loader.Error
        stdout: SplitParser {
            onRead: data => {
                try {
                    root.claimed = JSON.parse(data).claimed === true;
                } catch (e) {}
            }
        }
        onExited: root.claimed = false
    }

    PanelWindow {
        id: window
        visible: root.hosting && root.claimed && DettivoState.pillShown
        color: "transparent"
        exclusiveZone: 0
        aboveWindows: true
        focusable: false
        anchors.top: root.atTop
        anchors.bottom: !root.atTop
        anchors.left: root.atLeft
        anchors.right: root.atRight
        margins.top: root.atTop ? DettivoState.osdMargin : 0
        margins.bottom: root.atTop ? 0 : DettivoState.osdMargin
        margins.left: root.atLeft ? DettivoState.osdMargin : 0
        margins.right: root.atRight ? DettivoState.osdMargin : 0
        implicitWidth: Math.max(1, Math.ceil(pill.implicitWidth))
        implicitHeight: Math.max(1, Math.ceil(pill.implicitHeight))
        mask: Region {}

        Loader {
            id: pill
            active: root.hosting
            source: Qt.resolvedUrl("DettivoPill.qml")
        }
    }
}
