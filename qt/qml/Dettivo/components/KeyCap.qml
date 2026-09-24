import QtQuick
import Dettivo

// A single keyboard-key visual, e.g. inside a shortcut hint ("⌘ K").
// Consumes only Theme tokens.
Rectangle {
    id: root

    property string text: ""

    implicitWidth: Math.max(Theme.controlHeight, label.implicitWidth + 2 * Theme.spacingSm)
    implicitHeight: Theme.controlHeight * 0.8
    radius: Theme.radius
    color: Theme.roleRaisedSurface
    border.width: Theme.stateNormalBorderWidth
    border.color: Theme.roleBorder

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        font.weight: Theme.typeProvisionalWeight
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.text
}
