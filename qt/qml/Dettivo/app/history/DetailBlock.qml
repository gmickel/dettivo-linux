import QtQuick
import Dettivo

// A bordered block of the detail (history.png): a tracked label on the
// left of its header, a chip and a tracked meta line on the right, the
// text under the hairline. Enhanced reads in the body weight; Raw in the
// light weight so the difference shows at a glance.
Rectangle {
    id: root

    property string label: ""
    property string meta: ""
    property string chip: ""
    property string body: ""
    property bool light: false

    border.color: Theme.roleHairline
    border.width: Theme.hairlineWidth
    color: "transparent"
    implicitHeight: header.height + Theme.hairlineWidth + text.implicitHeight + Theme.space4 * 2
    radius: Theme.radius

    Item {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: Theme.controlHeight + Theme.space1

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4 - Theme.spacingXs
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.space3

            Chip {
                accent: true
                anchors.verticalCenter: parent.verticalCenter
                text: root.chip
                visible: root.chip.length > 0
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleMutedText
                font.capitalization: Font.AllUppercase
                font.family: Theme.fontFamily
                font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
                font.pixelSize: Theme.typeLabelSize
                text: root.meta
                visible: root.meta.length > 0
                Accessible.role: Accessible.StaticText
                Accessible.name: root.meta
            }
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    Text {
        id: text
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        anchors.top: header.bottom
        anchors.topMargin: Theme.space4
        color: root.light ? Theme.roleMutedText : Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: root.light ? Theme.typeProvisionalWeight : Font.Normal
        lineHeight: 1.4
        text: root.body
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.body
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.label
}
