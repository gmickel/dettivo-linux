import QtQuick
import Dettivo

// A group of rows on the settings pattern: the tracked label over a
// hairline, then the rows the section adds as children.
Column {
    id: root

    property string title: ""
    default property alias rows: body.data

    visible: body.children.some(child => child.key === undefined || SettingsUi.show(child.key))
    spacing: 0
    width: parent ? parent.width : implicitWidth

    Item {
        height: Theme.space5 + Theme.space1 + Theme.space3
        width: parent.width

        SectionLabel {
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.space2
            anchors.left: parent.left
            bottomPadding: 0
            leftPadding: 0
            text: root.title
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    Column {
        id: body
        spacing: 0
        width: parent.width
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: root.title
}
