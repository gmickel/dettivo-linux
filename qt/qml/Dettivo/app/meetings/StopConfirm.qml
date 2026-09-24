import QtQuick
import QtQuick.Controls
import Dettivo

// Stop asks once (states-and-hint-sheet.png): the state sentence, what
// follows (the takes close, the transcript is built, the speaker pass
// and the analysis run), and the action in the urgent colour as text.
// A refusal from the daemon lands in the dialog.
Popup {
    id: root

    property var live: null
    property string error: ""

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 7

    onOpened: root.error = ""

    Connections {
        target: root.live
        function onChanged() {
            if (root.live && root.live.finishing)
                root.close();
        }
        function onFailed(action, reason) {
            if (action === "stop")
                root.error = reason;
        }
    }

    function stop() {
        if (root.live)
            root.live.stop();
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space3

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Stop confirmation")

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
                text: qsTr("Stop this meeting?")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("Stop this meeting?")
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: qsTr("Both takes close, the transcript is built from them, then the speaker pass and the analysis run. The recording cannot resume.")
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
            Accessible.name: qsTr("Stop refused")
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.space5

            Button {
                text: qsTr("Keep recording")
                onClicked: root.close()
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleUrgent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: qsTr("Stop meeting")

                TapHandler {
                    onTapped: root.stop()
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        root.stop();
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Stop meeting")
                Accessible.onPressAction: root.stop()
            }
        }
    }
}
