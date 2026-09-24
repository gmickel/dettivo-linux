// Dettivo style — TabButton (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.TabButton {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    readonly property string underlineState: StyleHelpers.pickBorderState(control.checked, control.visualFocus, control.hovered)

    Accessible.role: Accessible.PageTab
    Accessible.name: control.text
    Accessible.selected: control.checked
    Accessible.onPressAction: control.click()

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.checked ? Theme.roleText : Theme.roleMutedText
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1.0 : 0.5
    }

    background: Rectangle {
        implicitHeight: Theme.controlHeight
        color: control.hovered || control.down ? Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.down ? "Pressed" : "Hover")) : "transparent"

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: StyleHelpers.borderWidth(control.underlineState)
            color: control.checked ? Theme.roleAccent : "transparent"
        }
    }
}
