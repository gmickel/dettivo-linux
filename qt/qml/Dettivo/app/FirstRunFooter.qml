import QtQuick
import QtQuick.Controls
import Dettivo

// The bottom of every first-run step (first-run-*.png): a hairline, a
// note on the left (rich text for the emphasised path), an optional quiet
// action beside it, and the step's buttons on the right.
Item {
    id: root

    property string note: ""
    property string noteRich: ""
    property string leftAction: ""
    property string secondary: ""
    property string primary: ""
    property bool primaryEnabled: true

    signal leftTriggered
    signal secondaryTriggered
    signal primaryTriggered

    implicitHeight: Theme.hairlineWidth + Theme.space5 + Math.max(noteText.implicitHeight, buttons.implicitHeight)

    // The footer's buttons hug their text (first-run-*.png: Skip is 49 px
    // wide), where the style's default gives every button three control
    // heights.
    function fitted(button) {
        return Math.max(Theme.controlHeight + Theme.space5 + Theme.space1, button.implicitContentWidth + button.leftPadding + button.rightPadding);
    }

    Rectangle {
        id: rule
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Text {
        id: noteText
        anchors.left: parent.left
        anchors.right: buttons.left
        anchors.rightMargin: Theme.space3
        anchors.top: rule.bottom
        anchors.topMargin: Theme.space5
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.noteRich.length > 0 ? root.noteRich : root.note
        textFormat: root.noteRich.length > 0 ? Text.StyledText : Text.PlainText
        visible: text.length > 0
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.note
    }

    Button {
        id: left
        anchors.left: parent.left
        anchors.top: rule.bottom
        anchors.topMargin: Theme.space5
        text: root.leftAction
        visible: root.leftAction.length > 0
        width: root.fitted(this)
        onClicked: root.leftTriggered()
    }

    Row {
        id: buttons
        anchors.right: parent.right
        anchors.top: rule.bottom
        anchors.topMargin: Theme.space5
        spacing: Theme.space3

        Button {
            text: root.secondary
            visible: root.secondary.length > 0
            width: root.fitted(this)
            onClicked: root.secondaryTriggered()
        }

        Button {
            enabled: root.primaryEnabled
            highlighted: root.primaryEnabled
            text: root.primary
            visible: root.primary.length > 0
            width: root.fitted(this)
            onClicked: root.primaryTriggered()
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("First run actions")
}
