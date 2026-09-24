import QtQuick
import Dettivo

// One of the three footer rows: a muted label on the left, the state on
// the right, accent when the state is warm.
Item {
    id: root

    property string role: ""
    property string label: ""
    property string value: ""
    property bool accent: false

    height: Theme.typeCaptionSize + Theme.space2
    width: parent ? parent.width : implicitWidth

    Text {
        anchors.left: parent.left
        anchors.right: valueText.left
        anchors.rightMargin: Theme.space2
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.label
    }

    Text {
        id: valueText
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        color: root.accent ? Theme.roleAccent : Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.value
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.role + ": " + root.label + ", " + root.value
}
