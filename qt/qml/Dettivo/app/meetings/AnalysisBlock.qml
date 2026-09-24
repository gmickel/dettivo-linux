pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// One block of the analysis (meeting-detail.png): the tracked label
// (`Summary`, `Decisions`, `Action items`) and either a paragraph or a
// list of dashes, in the body type at a comfortable line height.
Column {
    id: root

    property string label: ""
    property string paragraph: ""
    property var items: []

    spacing: Theme.space2

    SectionLabel {
        leftPadding: 0
        text: root.label
    }

    Text {
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        lineHeight: 1.5
        text: root.paragraph
        textFormat: Text.PlainText
        visible: root.paragraph.length > 0
        width: parent.width
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.paragraph
    }

    Repeater {
        model: root.items

        delegate: Text {
            id: line
            required property string modelData
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            lineHeight: 1.4
            text: qsTr("– %1").arg(line.modelData)
            textFormat: Text.PlainText
            width: root.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.ListItem
            Accessible.name: line.modelData
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.label
}
