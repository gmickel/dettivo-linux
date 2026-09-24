// Dettivo style — CheckBox (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.CheckBox {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    readonly property string fillState: StyleHelpers.pickFillState(control.down, control.checkState !== Qt.Unchecked, control.visualFocus, control.hovered)
    readonly property string borderState: StyleHelpers.pickBorderState(control.checkState !== Qt.Unchecked, control.visualFocus, control.hovered)

    Accessible.role: Accessible.CheckBox
    Accessible.name: control.text
    Accessible.checkable: true
    Accessible.checked: control.checkState === Qt.Checked

    indicator: Rectangle {
        implicitWidth: Theme.fontBaseSize + Theme.spacingSm
        implicitHeight: Theme.fontBaseSize + Theme.spacingSm

        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: Theme.radius > 0 ? Math.min(Theme.radius, width / 2) : 0

        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.fillState))
        border.width: StyleHelpers.borderWidth(control.borderState)
        border.color: StyleHelpers.borderColor(control.borderState)

        Rectangle {
            // check mark drawn as a simple bar pair (no icon asset needed here)
            visible: control.checkState === Qt.Checked
            anchors.centerIn: parent
            width: parent.width * 0.5
            height: Math.max(2, Theme.stateSelectedBorderWidth * 2)
            rotation: 45
            color: Theme.roleAccent
        }
        Rectangle {
            visible: control.checkState === Qt.PartiallyChecked
            anchors.centerIn: parent
            width: parent.width * 0.55
            height: Math.max(2, Theme.stateSelectedBorderWidth * 2)
            color: Theme.roleAccent
        }
    }

    contentItem: Text {
        leftPadding: control.indicator ? control.indicator.width + control.spacing : 0
        wrapMode: Text.WordWrap
        text: control.text
        font: control.font
        color: Theme.roleText
        verticalAlignment: Text.AlignVCenter
        opacity: control.enabled ? 1.0 : 0.5
    }
}
