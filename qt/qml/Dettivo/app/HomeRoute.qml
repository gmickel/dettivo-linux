import QtQuick
import Dettivo

// Home (home.png): a status sentence, not a dashboard. The sentence with
// the real hotkeys and the app the next take lands in, the instrument
// strip, Today as a plain list, and the rail with the engines and the
// agent surfaces.
Item {
    id: root

    property var router: null
    property var status: null
    property var engines: null
    property var today: null
    property var config: null
    property bool meetingActive: false
    signal meetingRequested

    readonly property string dictationState: root.status ? root.status.dictationState : "idle"
    readonly property string daemonState: root.status ? root.status.daemonState : "connecting"
    readonly property string hold: root.status && root.status.holdChord.length > 0 ? root.status.holdChord : "F9"
    readonly property string toggle: root.status && root.status.toggleChord.length > 0 ? root.status.toggleChord : "Super+Ctrl+X"
    readonly property string target: root.status && root.status.targetApp.length > 0 ? root.status.targetApp : qsTr("the focused app")
    readonly property string error: root.status && root.status.dictationError ? root.status.dictationError : ""

    readonly property string sentence: {
        if (root.daemonState === "away")
            return qsTr("Daemon unavailable.");
        if (root.status && root.daemonState === "connected" && !root.status.healthOk)
            return qsTr("Configuration needs attention.");
        if (root.error.length > 0 && root.dictationState === "idle")
            return qsTr("Dictation unavailable.");
        switch (root.dictationState) {
        case "recording":
            return qsTr("Listening.");
        case "transcribing":
            return qsTr("Transcribing.");
        case "inserting":
            return qsTr("Inserting.");
        default:
            return qsTr("Ready.");
        }
    }
    readonly property string detailPlain: root.error.length > 0 ? root.error : qsTr("Hold %1 to dictate into %2. %3 toggles.").arg(root.hold).arg(root.target).arg(root.toggle)
    readonly property string detailRich: root.error.length > 0 ? Html.escaped(root.error) : qsTr("Hold %1 to dictate into %2. %3 toggles.").arg(root.emphasis(root.hold)).arg(root.emphasis(root.target)).arg(root.emphasis(root.toggle))

    function emphasis(text) {
        return "<font color=\"" + Theme.roleText + "\">" + Html.escaped(text) + "</font>";
    }

    Shortcut {
        sequence: "J"
        enabled: root.visible
        onActivated: todayList.move(1)
    }
    Shortcut {
        sequence: "K"
        enabled: root.visible
        onActivated: todayList.move(-1)
    }

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY
        subtitlePlain: root.detailPlain
        subtitleRich: root.detailRich
        title: root.sentence
        trailing: root.status ? root.status.clock : ""
    }

    InstrumentStrip {
        id: strip
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space6
        meetingActive: root.meetingActive
        config: root.config
        status: root.status
        onMeetingRequested: root.meetingRequested()
    }

    TodayList {
        id: todayList
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: rail.left
        anchors.rightMargin: Theme.space7
        anchors.top: strip.bottom
        anchors.topMargin: Theme.space6
        model: root.today
        router: root.router
        status: root.status
    }

    RightRail {
        id: rail
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: strip.bottom
        anchors.topMargin: Theme.space6
        engines: root.engines
        status: root.status
        width: Theme.rightRailWidth
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Home")
}
