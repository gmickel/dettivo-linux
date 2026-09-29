// Dettivo style — ProgressBar (R3). Indeterminate mode uses a looping
// sweep driven by Motion's shimmer duration/easing rather than a literal
// animation timing. The sweep enters from the left of the track and
// leaves past its right, so the content clips it to the bar (fn-76):
// unclipped it drew across whatever sat beside the bar.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.ProgressBar {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, Theme.spacingSm)

    Accessible.role: Accessible.ProgressBar
    Accessible.name: qsTr("Progress")

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 5
        implicitHeight: Theme.spacingSm
        radius: Theme.radius
        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor("Normal"))
    }

    contentItem: Item {
        clip: true
        implicitWidth: Theme.controlHeight * 5
        implicitHeight: Theme.spacingSm

        Rectangle {
            id: fill
            visible: !control.indeterminate
            height: parent.height
            radius: Theme.radius
            width: parent.width * control.position
            color: Theme.roleAccent
        }

        Rectangle {
            id: sweep
            visible: control.indeterminate && control.visible
            width: parent.width * 0.3
            height: parent.height
            radius: Theme.radius
            color: Theme.roleAccent

            SequentialAnimation on x {
                running: sweep.visible && !Motion.reducedMotion
                loops: Animation.Infinite
                NumberAnimation {
                    from: -sweep.width
                    to: control.width
                    duration: Motion.durationShimmer
                    easing.type: Motion.easingLinear
                }
            }
        }
    }
}
