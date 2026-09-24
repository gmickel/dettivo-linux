// Dettivo style — Switch (R3). No dedicated QAccessible::Switch role exists,
// so this maps to Accessible.Button per Qt's own QQuickSwitch convention.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Switch {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    readonly property string trackBorderState: StyleHelpers.pickBorderState(control.checked, control.visualFocus, control.hovered)
    readonly property string knobFillState: StyleHelpers.pickFillState(control.down, control.checked, control.visualFocus, control.hovered)

    Accessible.role: Accessible.Button
    Accessible.name: control.text
    Accessible.ignored: !control.visible
    Accessible.checkable: true
    Accessible.checked: control.checked
    Accessible.onToggleAction: control.click()

    indicator: Rectangle {
        implicitWidth: Theme.controlHeight * 1.6
        implicitHeight: Theme.controlHeight * 0.7

        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: Theme.radius

        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.checked ? "Selected" : "Normal"))
        border.width: StyleHelpers.borderWidth(control.trackBorderState)
        border.color: StyleHelpers.borderColor(control.trackBorderState)
        opacity: control.enabled ? 1.0 : 0.5

        Rectangle {
            readonly property int travel: parent.width - width - 2 * Theme.stateNormalBorderWidth
            x: Theme.stateNormalBorderWidth + control.visualPosition * travel
            y: (parent.height - height) / 2
            width: parent.height - 2 * Theme.stateNormalBorderWidth
            height: width
            radius: Theme.radius
            color: control.checked ? Theme.roleAccent : Theme.roleMutedText

            Behavior on x {
                enabled: !control.down && !Motion.reducedMotion
                NumberAnimation {
                    duration: Motion.durationPill
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Motion.easingEnter
                }
            }
        }
    }

    contentItem: Text {
        leftPadding: control.indicator ? control.indicator.width + control.spacing : 0
        text: control.text
        font: control.font
        color: Theme.roleText
        verticalAlignment: Text.AlignVCenter
        opacity: control.enabled ? 1.0 : 0.5
    }
}
