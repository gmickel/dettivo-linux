import QtQuick
import Dettivo

// The analysis card of the live meeting (meeting-live.png): a quiet
// bordered card that says when the analysis runs and that it never
// overwrites the notes. The model's name comes from the engines the
// sidebar already lists; the card names the moment, not a promise.
Rectangle {
    id: root

    property string model: qsTr("the language model")
    property bool automatic: true

    border.color: Theme.roleHairline
    border.width: Theme.hairlineWidth
    color: "transparent"
    implicitHeight: header.height + body.implicitHeight + Theme.space4 * 2
    radius: Theme.radius

    Item {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        anchors.top: parent.top
        height: Theme.controlHeight

        SectionLabel {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            leftPadding: 0
            text: qsTr("Analysis")
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 0
            text: root.automatic ? qsTr("After stop") : qsTr("On request")
        }
    }

    Text {
        id: body
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        anchors.top: header.bottom
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        lineHeight: 1.4
        text: root.automatic ? qsTr("Summary, decisions and action items run on %1 once the recording ends. Notes stay yours; analysis never overwrites them.").arg(root.model) : qsTr("Summary, decisions and action items run on %1 when you ask for them in the meeting. Notes stay yours; analysis never overwrites them.").arg(root.model)
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Analysis card")
}
