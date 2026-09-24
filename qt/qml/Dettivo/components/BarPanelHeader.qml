import QtQuick
import Dettivo

// The panel's state header: the mark, "Dettivo", the status sentence and
// the REC chip while a take or a meeting runs; in the hint and unavailable
// states the sentence is the title and the reason sits under it.
Item {
    id: root

    required property var panel

    readonly property bool busy: root.panel.recording || root.panel.meeting

    implicitHeight: content.implicitHeight + Theme.space4 * 2

    Row {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.rowPaddingX
        anchors.rightMargin: Theme.rowPaddingX
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space3

        BarGlyph {
            id: glyph
            anchors.top: parent.top
            anchors.topMargin: Theme.space1
            state: root.panel.glyphState
            dimmed: !root.panel.live
            reducedMotion: root.panel.reducedMotion
        }

        Column {
            id: texts
            width: parent.width - glyph.width - (badge.visible ? badge.width + parent.spacing : 0) - parent.spacing
            spacing: Theme.space1

            Text {
                id: title
                width: parent.width
                text: root.panel.live ? qsTr("Dettivo") : root.panel.sentence
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                elide: Text.ElideRight
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }

            Text {
                id: status
                width: parent.width
                text: root.panel.live ? root.panel.sentence : root.panel.reason
                color: root.panel.unavailable ? Theme.roleUrgent : Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.features: Theme.typeTabularNumerals
                elide: Text.ElideRight
                wrapMode: root.panel.live ? Text.NoWrap : Text.WordWrap
                maximumLineCount: root.panel.live ? 1 : 3
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }

        Chip {
            id: badge
            visible: root.busy
            anchors.top: parent.top
            accent: true
            dotColor: Theme.roleAccent
            text: qsTr("REC")
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Theme.hairlineWidth
        color: Theme.roleHairline
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: root.panel.sentence
}
