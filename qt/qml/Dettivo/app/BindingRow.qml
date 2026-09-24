pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// One binding of the Keys step (first-run-1-keys.png): the action, the
// chord as key caps joined by plus signs with a muted hint after it, and
// the command it runs, on a hairline.
Item {
    id: root

    property string action: ""
    property var keys: []
    property string hint: ""
    property string runs: ""

    implicitHeight: Theme.firstRunRowHeight

    Text {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        text: root.action
        Accessible.role: Accessible.StaticText
        Accessible.name: root.action
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: Theme.firstRunActionColumn
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Repeater {
            model: root.keys

            delegate: Row {
                id: cap
                required property int index
                required property string modelData
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.space2

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleFaintText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    text: "+"
                    visible: cap.index > 0
                    Accessible.role: Accessible.StaticText
                    Accessible.name: qsTr("plus")
                }

                KeyCap {
                    anchors.verticalCenter: parent.verticalCenter
                    text: cap.modelData
                }
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            leftPadding: Theme.space2
            text: root.hint
            visible: root.hint.length > 0
            Accessible.role: Accessible.StaticText
            Accessible.name: root.hint
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: root.width - Theme.firstRunRunsColumn
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.runs
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.runs
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.action + ": " + root.keys.join("+")
}
