import QtQuick
import Dettivo

// The block at the foot of every settings route (settings-models.png):
// the tracked label and the bordered box with the keys the route writes,
// as TOML with the values in force, so the file is never a hidden second
// system (FR-C6).
Item {
    id: root

    property string text: ""

    implicitHeight: label.implicitHeight + Theme.space2 + box.height

    SectionLabel {
        id: label
        anchors.left: parent.left
        anchors.top: parent.top
        bottomPadding: 0
        leftPadding: 0
        text: qsTr("This route writes")
        topPadding: 0
    }

    Rectangle {
        id: box
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: label.bottom
        anchors.topMargin: Theme.space2
        border.color: Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: body.implicitHeight + Theme.space4 * 2
        radius: Theme.radius

        Text {
            id: body
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            color: Theme.roleMutedText
            elide: Text.ElideNone
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            lineHeight: 1.35
            text: root.text
            wrapMode: Text.WrapAnywhere
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Configuration keys")
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Keys this route writes")
}
