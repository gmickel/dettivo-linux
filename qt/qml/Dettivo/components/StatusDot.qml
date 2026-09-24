import QtQuick
import Dettivo

// A small colored status indicator (e.g. recording / connected / idle).
// `active` drives a soft pulse (skipped entirely under reduced motion).
Item {
    id: root

    enum Status {
        Neutral,
        Accent,
        Selected,
        Urgent
    }

    property int status: StatusDot.Neutral
    property bool active: false
    property string label: ""

    readonly property color dotColor: {
        switch (status) {
        case StatusDot.Accent:
            return Theme.roleAccent;
        case StatusDot.Selected:
            return Theme.roleSelected;
        case StatusDot.Urgent:
            return Theme.roleUrgent;
        default:
            return Theme.roleMutedText;
        }
    }

    implicitWidth: Theme.spacingSm
    implicitHeight: Theme.spacingSm

    Rectangle {
        id: dot
        anchors.centerIn: parent
        width: Theme.spacingSm
        height: Theme.spacingSm
        radius: width / 2
        color: root.dotColor

        SequentialAnimation on opacity {
            running: root.active && !Motion.reducedMotion
            loops: Animation.Infinite
            NumberAnimation {
                from: 1.0
                to: 0.35
                duration: Motion.durationShimmer / 2
                easing.type: Motion.easingLinear
            }
            NumberAnimation {
                from: 0.35
                to: 1.0
                duration: Motion.durationShimmer / 2
                easing.type: Motion.easingLinear
            }
        }
    }

    Accessible.role: Accessible.Indicator
    Accessible.name: root.label.length > 0 ? root.label : qsTr("status")
}
