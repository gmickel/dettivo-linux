import QtQuick
import Dettivo

// One source in the live header (meeting-live.png): the tracked label
// (`MIC`, `SYSTEM`), five level bars that follow `audio.level` for that
// source, and the device or app name beside them in muted.
Item {
    id: root

    property string label: ""
    property string device: ""
    property real level: 0
    property real peak: 0

    implicitHeight: Theme.typeBodySize + Theme.space1
    implicitWidth: row.implicitWidth

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
            font.pixelSize: Theme.typeLabelSize
            text: root.label
        }

        LevelBars {
            anchors.verticalCenter: parent.verticalCenter
            label: qsTr("%1 level").arg(root.label)
            level: root.level
            peak: root.peak
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.device
            width: Math.min(implicitWidth, Theme.space8 * 4)
            Accessible.role: Accessible.StaticText
            Accessible.name: root.device
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("%1 meter").arg(root.label)
}
