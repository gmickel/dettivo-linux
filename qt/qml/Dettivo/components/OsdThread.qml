import QtQuick
import Dettivo

// The enhancing thread: one gold line that travels the width while the
// rewrite runs, drawn as a gradient rectangle on the scene graph. Under
// reduced motion the thread stands still at full length.
Item {
    id: root

    property bool running: false

    implicitWidth: Theme.rowHeight * 3 + Theme.space5
    implicitHeight: Theme.space1

    Rectangle {
        id: track
        anchors.fill: parent
        radius: Theme.radius
        color: Qt.alpha(Theme.roleAccent, 0.18)
    }

    Rectangle {
        id: thread
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height
        width: root.running ? parent.width / 2 : parent.width
        radius: Theme.radius
        x: 0
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0.0
                color: Qt.alpha(Theme.roleAccent, 0.0)
            }
            GradientStop {
                position: 1.0
                color: Theme.roleAccent
            }
        }

        SequentialAnimation on x {
            running: root.running
            loops: Animation.Infinite
            NumberAnimation {
                from: -thread.width
                to: root.width
                duration: Motion.durationShimmer
                easing.type: Motion.easingLinear
            }
        }

        onWidthChanged: {
            if (!root.running)
                x = 0;
        }
    }

    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Enhancing")
}
