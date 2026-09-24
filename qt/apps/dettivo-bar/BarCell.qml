import QtQuick
import Dettivo

// One cell of the bar glyph strip in osd.png: the 16 px mark on the bar's
// own background with the state's name in caps beside it, the way the
// artboard presents each state. The item's `state` is the glyph's.
Item {
    id: root

    readonly property string caption: {
        switch (root.state) {
        case "listening":
            return qsTr("Listening");
        case "transcribing":
            return qsTr("Transcribing");
        case "meeting":
            return qsTr("Meeting");
        case "error":
            return qsTr("Error");
        default:
            return qsTr("Idle");
        }
    }

    state: "idle"
    implicitWidth: row.implicitWidth + Theme.space4 * 2
    implicitHeight: Theme.rowHeight + Theme.space2

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.space3

        BarGlyph {
            anchors.verticalCenter: parent.verticalCenter
            state: root.state
            elapsed: root.state === "meeting" ? "23:41" : ""
            level: 0.7
            reducedMotion: true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.caption
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeLabelSize
            font.capitalization: Font.AllUppercase
            font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: root.caption
}
