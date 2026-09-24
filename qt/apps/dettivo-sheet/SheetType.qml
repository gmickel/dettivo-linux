import QtQuick
import QtQuick.Layouts
import Dettivo

// The type scale from display to tracked label, plus the provisional weight.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Type · %1 via the monospace alias").arg(Theme.fontFamily)
    }
    Rectangle {
        Layout.fillWidth: true
        border.color: Theme.roleHairline
        border.width: Theme.hairlineWidth
        color: "transparent"
        implicitHeight: column.implicitHeight + Theme.space6 * 2

        ColumnLayout {
            id: column
            anchors.fill: parent
            anchors.margins: Theme.space6
            spacing: Theme.space3

            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.features: Theme.typeTabularNumerals
                font.pixelSize: Theme.typeDisplaySize
                font.weight: Theme.typeEmphasisWeight
                text: "00:23:41"
            }
            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeTitleSize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Dettivo Linux kickoff")
            }
            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeHeadingSize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Recent dictations")
            }
            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: qsTr("Add a regression test for the merger overlap case before we ship.")
            }
            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("ghostty · Enhanced · 4 s")
            }
            SectionLabel {
                text: qsTr("Workspace")
            }
            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeProvisionalWeight
                text: qsTr("Light 300 for provisional transcript text")
            }
        }
    }
}
