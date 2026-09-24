import QtQuick
import QtQuick.Controls
import Dettivo

// One MCP host (agents.png): the host, the file it reads with the entry
// key beside it, the state box and the write action, which runs
// `dettivo mcp config --host <id> --write`.
Item {
    id: root

    property string hostId: ""
    property string name: ""
    property string path: ""
    property string entryKey: ""
    property bool configured: false
    property string stateText: ""
    property bool busy: false

    signal write

    readonly property int pathColumn: Theme.space8 * 4 + Theme.space1
    readonly property int stateColumn: Theme.space8 * 11 + Theme.space8 / 2 + Theme.space2

    implicitHeight: Theme.settingsBindingRowHeight

    Text {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        text: root.name
        Accessible.role: Accessible.StaticText
        Accessible.name: root.name
    }

    Text {
        anchors.left: parent.left
        visible: SettingsUi.advanced
        anchors.leftMargin: root.pathColumn
        anchors.right: stateBox.left
        anchors.rightMargin: Theme.space4
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        elide: Text.ElideMiddle
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.configured ? qsTr("%1 · %2").arg(root.path).arg(root.entryKey) : root.path
        Accessible.role: Accessible.StaticText
        Accessible.name: root.path
    }

    Rectangle {
        id: stateBox
        anchors.left: parent.left
        anchors.leftMargin: root.stateColumn
        anchors.verticalCenter: parent.verticalCenter
        border.color: root.configured ? Theme.roleAccent : Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: Theme.controlHeight - Theme.space2
        radius: Theme.radius
        width: Theme.settingsStateWidth

        Text {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space3
            anchors.verticalCenter: parent.verticalCenter
            color: root.configured ? Theme.roleAccent : Theme.roleMutedText
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeLabelSize * 0.08
            font.pixelSize: Theme.typeLabelSize
            text: root.stateText
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("%1 state: %2").arg(root.name).arg(root.stateText)
        }
    }

    Button {
        anchors.left: stateBox.right
        anchors.leftMargin: Theme.space4
        anchors.verticalCenter: parent.verticalCenter
        enabled: !root.busy
        text: root.configured ? qsTr("Reconnect") : qsTr("Connect")
        onClicked: root.write()
        Accessible.name: qsTr("Write %1 config").arg(root.name)
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.name
}
