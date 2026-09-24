pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Dettivo

// Sixteen palette roles, derived once from the theme.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Palette · roles derived once from the theme")
    }
    GridLayout {
        Layout.fillWidth: true
        columnSpacing: Theme.space5
        columns: 8
        rowSpacing: Theme.space4

        Repeater {
            model: [
                {
                    name: "background",
                    color: Theme.roleSurface
                },
                {
                    name: "surface raised",
                    color: Theme.roleRaisedSurface
                },
                {
                    name: "foreground",
                    color: Theme.roleText
                },
                {
                    name: "text muted",
                    color: Theme.roleMutedText
                },
                {
                    name: "border",
                    color: Theme.roleBorder
                },
                {
                    name: "accent",
                    color: Theme.roleAccent
                },
                {
                    name: "accent selected",
                    color: Theme.roleSelected
                },
                {
                    name: "urgent",
                    color: Theme.roleUrgent
                },
                {
                    name: "bar",
                    color: Theme.colorBar
                },
                {
                    name: "speaker · you",
                    color: Theme.roleSpeakerColors[0]
                },
                {
                    name: "speaker 1",
                    color: Theme.roleSpeakerColors[1]
                },
                {
                    name: "speaker 2",
                    color: Theme.roleSpeakerColors[2]
                },
                {
                    name: "speaker 3",
                    color: Theme.roleSpeakerColors[3]
                },
                {
                    name: "speaker 4",
                    color: Theme.roleSpeakerColors[4]
                },
                {
                    name: "highlight",
                    color: Theme.roleHighlight
                },
                {
                    name: "cursor",
                    color: Theme.colorCursor
                }
            ]

            delegate: ColumnLayout {
                id: swatch
                required property var modelData
                Layout.fillWidth: true
                spacing: Theme.space2

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: Theme.controlHeight + Theme.space5
                    border.color: Theme.roleHairline
                    border.width: Theme.hairlineWidth
                    color: swatch.modelData.color
                    radius: Theme.radius
                }
                Text {
                    color: Theme.roleMutedText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeCaptionSize
                    text: swatch.modelData.name
                }
            }
        }
    }
}
