import QtQuick
import QtQuick.Controls
import Dettivo

// Delete asks once (states-and-hint-sheet.png): the state sentence, what
// goes under the configured policy and what stays, and the action as
// text in the urgent colour, never a red button. A refusal (the meeting
// still recording, a job on it) names the daemon's reason.
Popup {
    id: root

    property var actions: null
    property string meetingId: ""
    property string error: ""

    readonly property string policy: root.actions ? root.actions.deletePolicy : "all"
    readonly property string policyLine: {
        if (root.policy === "transcript_only")
            return qsTr("The transcript, its speakers and the analysis leave the store; the notes and the audio stay ([meetings] delete_artifact_policy).");
        if (root.policy === "transcript_and_audio")
            return qsTr("The transcript, the analysis and both takes leave the store; the facts and the notes stay ([meetings] delete_artifact_policy).");
        return qsTr("The meeting, its takes, notes and analysis leave the store. Nothing brings them back ([meetings] delete_artifact_policy).");
    }

    function openFor(id) {
        root.meetingId = id;
        root.error = "";
        root.open();
    }

    function remove() {
        if (root.actions)
            root.actions.remove(root.meetingId, "");
    }

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 7

    Connections {
        target: root.actions
        function onRemoved(meetingId) {
            if (meetingId === root.meetingId)
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
        Accessible.name: qsTr("Delete meeting confirmation")

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
                text: qsTr("Delete this meeting?")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("Delete this meeting?")
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.policyLine
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
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleUrgent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: qsTr("Delete meeting for good")

                TapHandler {
                    onTapped: root.remove()
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        root.remove();
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Delete meeting for good")
                Accessible.onPressAction: root.remove()
            }
        }
    }
}
