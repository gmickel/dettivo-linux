import QtQuick
import QtQuick.Controls
import Dettivo

// The notes (meeting-live.png, meeting-detail.png): a bordered Markdown
// editor with a live caret, the tracked `Notes` label over it and the
// save state (`Markdown · Saved`) on the right. The text reaches the
// model as it is typed; the model writes it after a second of quiet.
Item {
    id: root

    property string text: ""
    property string saveState: ""
    property string placeholder: qsTr("## Decisions\n- ")
    readonly property bool typing: editor.activeFocus

    signal edited(string markdown)

    onTextChanged: {
        if (editor.text !== root.text)
            editor.text = root.text;
    }

    function takeFocus() {
        editor.forceActiveFocus();
        editor.cursorPosition = editor.length;
    }

    Item {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: Theme.controlHeight

        SectionLabel {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            leftPadding: 0
            text: qsTr("Notes")
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 0
            text: root.saveState.length > 0 ? qsTr("Markdown · %1").arg(root.saveState) : qsTr("Markdown")
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        border.color: editor.activeFocus ? Qt.alpha(Theme.stateFocusColor, Theme.stateFocusBorderAlpha) : Theme.roleHairline
        border.width: editor.activeFocus ? Theme.stateFocusBorderWidth : Theme.hairlineWidth
        color: "transparent"
        radius: Theme.radius

        Flickable {
            id: flick
            anchors.fill: parent
            anchors.margins: Theme.space3
            clip: true
            contentHeight: editor.implicitHeight
            contentWidth: width

            TextArea.flickable: TextArea {
                id: editor
                cursorVisible: activeFocus
                font.pixelSize: Theme.typeBodySize
                placeholderText: root.placeholder
                text: root.text
                wrapMode: TextEdit.Wrap
                onTextChanged: {
                    if (editor.text !== root.text)
                        root.edited(editor.text);
                }
                Accessible.role: Accessible.EditableText
                Accessible.name: qsTr("Notes editor")
            }

            ScrollBar.vertical: ScrollBar {}
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Notes")
}
