import QtQuick
import Dettivo

// The daemon-unavailable state (fn-17 R5), over every page: the urgent
// square, one sentence, the systemd hint. It appears once the daemon has
// been away for the grace period and clears the moment it answers again;
// nothing else in the window changes.
Rectangle {
    id: root

    property var status: null

    readonly property bool away: root.status ? root.status.daemonState === "away" : false
    readonly property bool unhealthy: root.status ? (root.status.daemonState === "connected" && !root.status.healthOk) : false
    readonly property bool shown: root.away || root.unhealthy
    readonly property string sentence: root.away ? qsTr("Daemon unavailable.") : qsTr("Configuration needs attention.")
    readonly property string reason: root.away ? qsTr("Start it with systemctl --user start dettivod.socket; the app reconnects on its own.") : qsTr("Fix config.toml; the daemon keeps the last valid settings until it parses.")

    color: Theme.roleRaisedSurface
    height: root.shown ? row.implicitHeight + Theme.space4 * 2 : 0
    implicitHeight: height
    visible: root.shown

    Behavior on height {
        enabled: !Motion.reducedMotion
        NumberAnimation {
            duration: Motion.duration(Motion.durationEnter)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingEnter
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Row {
        id: row
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space3

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleUrgent
            height: Theme.space3
            width: Theme.space3
        }

        Text {
            id: title
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.weight: Theme.typeEmphasisWeight
            text: root.sentence
            Accessible.role: Accessible.StaticText
            Accessible.name: root.away ? qsTr("Daemon unavailable") : qsTr("Configuration needs attention")
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.reason
            width: Math.max(0, row.width - title.implicitWidth - Theme.space3 * 3)
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Daemon state")
}
