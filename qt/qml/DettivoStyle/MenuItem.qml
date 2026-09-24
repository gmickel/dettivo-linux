// Dettivo style — MenuItem (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.MenuItem {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    Accessible.role: Accessible.MenuItem
    Accessible.name: control.text

    indicator: Rectangle {
        implicitWidth: Theme.fontBaseSize * 0.6
        implicitHeight: Theme.fontBaseSize * 0.6
        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: width / 2
        visible: control.checkable && control.checked
        color: Theme.roleAccent
    }

    contentItem: Text {
        readonly property real indicatorPadding: control.checkable && control.indicator ? control.indicator.width + control.spacing : 0
        leftPadding: indicatorPadding
        text: control.text
        font: control.font
        color: Theme.roleText
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1.0 : 0.5
    }

    arrow: Item {
        x: control.width - width - control.rightPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitWidth: Theme.fontBaseSize * 0.5
        implicitHeight: Theme.fontBaseSize * 0.5
        visible: control.subMenu

        Rectangle {
            width: parent.width * 0.7
            height: Math.max(1, Theme.stateNormalBorderWidth)
            color: Theme.roleMutedText
            anchors.centerIn: parent
            y: -height * 0.28
            rotation: 45
        }
        Rectangle {
            width: parent.width * 0.7
            height: Math.max(1, Theme.stateNormalBorderWidth)
            color: Theme.roleMutedText
            anchors.centerIn: parent
            y: height * 0.28
            rotation: -45
        }
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 5
        implicitHeight: Theme.controlHeight
        radius: Theme.radius

        readonly property string fillState: StyleHelpers.pickFillState(control.down, control.highlighted, control.visualFocus, control.hovered)
        color: control.down || control.highlighted || control.hovered || control.visualFocus ? Qt.tint(Theme.roleRaisedSurface, StyleHelpers.fillColor(fillState)) : "transparent"
    }
}
