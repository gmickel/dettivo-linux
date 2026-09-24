import QtQuick
import Dettivo

// A section of a page (Today, Engines, Agents): the heading, a tracked
// label on the right and the hairline under both.
Item {
    id: root

    property string title: ""
    property string trailing: ""

    implicitHeight: heading.implicitHeight + Theme.space3 + Theme.hairlineWidth

    Text {
        id: heading
        anchors.left: parent.left
        anchors.top: parent.top
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeHeadingSize
        font.weight: Theme.typeEmphasisWeight
        text: root.title
        Accessible.role: Accessible.Heading
        Accessible.name: root.title
    }

    SectionLabel {
        anchors.bottom: heading.bottom
        anchors.right: parent.right
        bottomPadding: 0
        text: root.trailing
        visible: root.trailing.length > 0
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }
}
