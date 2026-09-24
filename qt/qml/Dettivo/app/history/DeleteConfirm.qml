import QtQuick
import QtQuick.Controls
import Dettivo

// Delete asks once (states-and-hint-sheet.png): the state sentence, what
// goes per the artifact policy, and the action as text in the urgent
// colour, never a red button. A refusal (a job still using the item)
// names the daemon's reason.
Popup {
    id: root

    property var actions: null
    property string itemId: ""
    property string error: ""

    function openFor(id) {
        root.itemId = id;
        root.error = "";
        root.open();
    }

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 7

    Connections {
        target: root.actions
        function onRemoved(id) {
            if (id === root.itemId)
                root.close();
        }
        function onFailed(action, reason) {
            if (action === "delete")
                root.error = reason;
        }
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space3

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Delete confirmation")

        Row {
            spacing: Theme.space3

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleUrgent
                height: Theme.space3
                width: Theme.space3
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Delete this dictation?")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("Delete this dictation?")
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: qsTr("The text and, per [history] artifacts, its take and metadata leave the store. Nothing brings them back.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Text {
            color: Theme.roleUrgent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.error
            visible: root.error.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Delete refused")
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.space5

            Button {
                text: qsTr("Keep it")
                onClicked: root.close()
            }

            Text {
                id: confirm
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleUrgent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: qsTr("Delete for good")

                TapHandler {
                    onTapped: {
                        if (root.actions)
                            root.actions.remove(root.itemId);
                    }
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        if (root.actions)
                            root.actions.remove(root.itemId);
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Delete for good")
                Accessible.onPressAction: {
                    if (root.actions)
                        root.actions.remove(root.itemId);
                }
            }
        }
    }
}
