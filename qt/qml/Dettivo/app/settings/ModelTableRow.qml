import QtQuick
import QtQuick.Controls
import Dettivo

// One model of the Models table (settings-models.png): the name with its
// engine and languages under it, the size, the backend its engine runs
// on, the state box (warm, idle, a percentage with the progress hairline,
// not downloaded) and the action its state allows.
Item {
    id: root

    property string provider: ""
    property string modelId: ""
    property string name: ""
    property string detail: ""
    property string size: ""
    property string backend: ""
    property string stateText: ""
    property real progress: 0
    property bool ready: false
    property bool downloading: false
    property bool selected: false
    property bool warm: false

    signal download
    signal cancel
    signal remove

    readonly property bool accent: root.warm || root.downloading || (root.ready && root.selected)
    readonly property int sizeColumn: Theme.space8 * 7 - Theme.space3
    readonly property int backendColumn: root.sizeColumn + Theme.space8 * 2 + Theme.space3
    readonly property int stateColumn: root.backendColumn + Theme.space8 * 2 + Theme.space7

    // A detail that needs a second line makes the row taller, as the
    // artboard's rows do.
    implicitHeight: Math.max(Theme.settingsModelRowHeight, nameColumn.implicitHeight + Theme.space4 * 2)

    Column {
        id: nameColumn
        anchors.left: parent.left
        anchors.right: parent.left
        anchors.rightMargin: -root.sizeColumn + Theme.space5
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space1

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.weight: Theme.typeEmphasisWeight
            text: root.name
            Accessible.role: Accessible.StaticText
            Accessible.name: root.name
        }

        Text {
            color: Theme.roleMutedText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            maximumLineCount: 2
            text: root.detail
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: root.detail
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: root.sizeColumn
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.size
        Accessible.role: Accessible.StaticText
        Accessible.name: root.size
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: root.backendColumn
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.backend
        Accessible.role: Accessible.StaticText
        Accessible.name: root.backend
    }

    Rectangle {
        id: stateBox
        anchors.left: parent.left
        anchors.leftMargin: root.stateColumn
        anchors.verticalCenter: parent.verticalCenter
        border.color: root.accent ? Theme.roleAccent : Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: Theme.controlHeight - Theme.space2
        radius: Theme.radius
        width: Theme.settingsStateWidth

        Text {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space3
            anchors.verticalCenter: parent.verticalCenter
            color: root.accent ? Theme.roleAccent : Theme.roleMutedText
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeLabelSize * 0.08
            font.pixelSize: Theme.typeLabelSize
            text: root.stateText
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("%1 state: %2").arg(root.modelId).arg(root.stateText)
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            color: Theme.roleAccent
            height: Theme.progressHairlineWidth
            visible: root.downloading
            width: parent.width * root.progress
        }
    }

    Row {
        anchors.left: stateBox.right
        anchors.leftMargin: Theme.space4
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Button {
            text: qsTr("Download")
            visible: !root.ready && !root.downloading
            onClicked: root.download()
            Accessible.name: qsTr("Download %1").arg(root.modelId)
        }

        Button {
            text: qsTr("Cancel")
            visible: root.downloading
            onClicked: root.cancel()
            Accessible.name: qsTr("Cancel %1").arg(root.modelId)
        }

        Button {
            enabled: !root.selected && !root.warm
            text: qsTr("Delete")
            visible: root.ready
            onClicked: root.remove()
            Accessible.name: qsTr("Delete %1").arg(root.modelId)
        }
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
