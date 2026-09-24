// Dettivo style — RadioButton (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.RadioButton {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    readonly property string fillState: StyleHelpers.pickFillState(control.down, control.checked, control.visualFocus, control.hovered)
    readonly property string borderState: StyleHelpers.pickBorderState(control.checked, control.visualFocus, control.hovered)

    Accessible.role: Accessible.RadioButton
    Accessible.name: control.text
    Accessible.checkable: true
    Accessible.checked: control.checked

    indicator: Rectangle {
        implicitWidth: Theme.fontBaseSize + Theme.spacingSm
        implicitHeight: Theme.fontBaseSize + Theme.spacingSm

        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: width / 2

        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.fillState))
        border.width: StyleHelpers.borderWidth(control.borderState)
        border.color: StyleHelpers.borderColor(control.borderState)

        Rectangle {
            visible: control.checked
            anchors.centerIn: parent
            width: parent.width * 0.45
            height: width
            radius: width / 2
            color: Theme.roleAccent
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
