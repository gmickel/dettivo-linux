import QtQuick
import QtQuick.Layouts
import Dettivo

// Title block: the sheet's name, where its tokens came from, and the mark.
RowLayout {
    spacing: Theme.space4

    ColumnLayout {
        Layout.fillWidth: true
        spacing: Theme.space2

        SectionLabel {
            text: qsTr("Dettivo for Linux · design system")
        }
        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
            font.pixelSize: Theme.typeDisplaySize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Studio")
        }
        Text {
            Layout.fillWidth: true
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: qsTr("Tokens resolved from the active theme (source: %1). Sharp corners, hairlines, one accent.").arg(Theme.source)
            wrapMode: Text.WordWrap
        }
    }
    Icon {
        accessibleName: qsTr("Dettivo mark")
        color: Theme.roleAccent
        size: Theme.space8
        source: "sixbar"
    }
}
