import QtQuick
import Dettivo

// A per-source input meter: a hairline track with an accent fill for the
// current level and a thin peak mark that decays. Sits beside a device or
// app name in the meeting instrument strip.
Item {
    id: root

    property real level: 0
    property real peak: 0
    property bool clipping: level >= 0.98
    property string label: qsTr("Input level")

    implicitWidth: Theme.controlHeight * 3
    implicitHeight: Theme.space2

    Rectangle {
        id: track
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.roleRaisedSurface
        border.width: Theme.hairlineWidth
        border.color: Theme.roleHairline
    }

    Rectangle {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: parent.width * Math.max(0, Math.min(1, root.level))
        radius: Theme.radius
        color: root.clipping ? Theme.roleUrgent : Theme.roleAccent

        Behavior on width {
            enabled: !Motion.reducedMotion
            NumberAnimation {
                duration: Motion.duration(Motion.levelFrameMs)
                easing.type: Motion.easingLinear
            }
        }
    }

    Rectangle {
        visible: root.peak > 0
        x: Math.max(0, parent.width * Math.min(1, root.peak) - width)
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: Theme.hairlineWidth
        color: Theme.roleText
    }

    Accessible.role: Accessible.ProgressBar
    Accessible.name: root.label
}
