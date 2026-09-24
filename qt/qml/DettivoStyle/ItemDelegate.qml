// Dettivo style — ItemDelegate (R3). Base row style reused by ComboBox's
// dropdown delegate and any host list needing a plain selectable row.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.ItemDelegate {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    Accessible.role: Accessible.ListItem
    Accessible.name: control.text
    Accessible.selectable: true
    Accessible.selected: control.highlighted

    contentItem: Text {
        text: control.text
        font: control.font
        color: Theme.roleText
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1.0 : 0.5
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 5
        implicitHeight: Theme.controlHeight
        radius: Theme.radius

        readonly property string fillState: StyleHelpers.pickFillState(control.down, control.highlighted, control.visualFocus, control.hovered)
        color: control.down || control.highlighted || control.hovered || control.visualFocus ? Qt.tint(Theme.roleRaisedSurface, StyleHelpers.fillColor(fillState)) : "transparent"
    }
}
