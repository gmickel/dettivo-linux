import QtQuick
import Dettivo

// The pill's state dot: a square on square themes, accent while listening,
// held (selected) while the engine works, urgent on an error. It breathes
// while listening unless motion is reduced.
Rectangle {
    id: root

    property bool pulsing: false

    width: Theme.space2 + Theme.space1
    height: width
    radius: Theme.radius
    color: Theme.roleAccent

    SequentialAnimation on opacity {
        running: root.pulsing
        loops: Animation.Infinite
        NumberAnimation {
            from: 1.0
            to: 0.55
            duration: Motion.durationShimmer / 2
            easing.type: Motion.easingLinear
        }
        NumberAnimation {
            from: 0.55
            to: 1.0
            duration: Motion.durationShimmer / 2
            easing.type: Motion.easingLinear
        }
    }

    onPulsingChanged: {
        if (!pulsing)
            opacity = 1.0;
    }

    Accessible.role: Accessible.Indicator
    Accessible.name: qsTr("Status dot")
}
