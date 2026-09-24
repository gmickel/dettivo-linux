// Dettivo style — ScrollBar (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.ScrollBar {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)

    padding: Theme.spacingXs
    visible: control.policy !== T.ScrollBar.AlwaysOff
    minimumSize: orientation === Qt.Horizontal ? (width > 0 ? Math.min(1, height / width) : 1) : (height > 0 ? Math.min(1, width / height) : 1)

    Accessible.role: Accessible.ScrollBar
    Accessible.name: control.orientation === Qt.Horizontal ? qsTr("Horizontal scroll bar") : qsTr("Vertical scroll bar")

    contentItem: Rectangle {
        implicitWidth: control.interactive ? Theme.spacingSm : Theme.spacingXs
        implicitHeight: control.interactive ? Theme.spacingSm : Theme.spacingXs
        radius: Theme.radius

        color: Qt.tint(Theme.roleMutedText, StyleHelpers.fillColor(control.pressed ? "Pressed" : (control.hovered ? "Hover" : "Normal")))
        opacity: 0.0

        states: State {
            name: "active"
            when: control.policy === T.ScrollBar.AlwaysOn || (control.active && control.size < 1.0)
            PropertyChanges {
                control.contentItem.opacity: 0.85
            }
        }

        transitions: Transition {
            from: "active"
            SequentialAnimation {
                PauseAnimation {
                    duration: Motion.duration(Motion.durationExit)
                }
                NumberAnimation {
                    target: control.contentItem
                    property: "opacity"
                    to: 0.0
                    duration: Motion.duration(Motion.durationExit)
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Motion.easingExit
                }
            }
        }
    }
}
