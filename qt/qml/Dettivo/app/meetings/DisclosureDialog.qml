import QtQuick
import QtQuick.Controls
import Dettivo

// The recording disclosure (import-and-disclosure.png): the message
// verbatim in a bordered box, the sentence that it is asked once and how
// agents pass it, Copy message, Not now and Acknowledge and start. Not
// now leaves the list without starting; Acknowledge and start records the
// acknowledgement on the start request.
Popup {
    id: root

    property var live: null
    property string message: ""

    // The text shown: what the start handed over, else the model's copy of
    // the notice (read after this dialog was asked to open by a render).
    readonly property string shownMessage: root.message.length > 0 ? root.message : (root.live ? root.live.disclosureMessage : "")

    function openWith(text) {
        root.message = text;
        root.open();
    }

    function dismiss() {
        if (root.live)
            root.live.dismissStart();
        root.close();
    }

    function acknowledge() {
        if (root.live)
            root.live.acknowledgeAndStart();
        root.close();
    }

    closePolicy: Popup.NoAutoClose
    modal: true
    padding: Theme.space5
    width: Theme.space8 * 10 + Theme.space6

    Connections {
        target: root.live
        function onStarted(meetingId) {
            root.close();
        }
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space4

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Recording disclosure")

        Item {
            height: heading.implicitHeight
            width: parent.width

            Text {
                id: heading
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeHeadingSize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Recording disclosure")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("Recording disclosure heading")
            }

            SectionLabel {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                rightPadding: 0
                text: qsTr("Once")
            }
        }

        Rectangle {
            border.color: Theme.roleBorder
            border.width: Theme.stateNormalBorderWidth
            color: Theme.roleRaisedSurface
            height: messageText.implicitHeight + Theme.space4 * 2
            radius: Theme.radius
            width: parent.width

            Text {
                id: messageText
                anchors.left: parent.left
                anchors.margins: Theme.space4
                anchors.right: parent.right
                anchors.top: parent.top
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                lineHeight: 1.5
                text: root.shownMessage
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Disclosure message")
                Accessible.description: root.shownMessage
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("Copy this into the meeting chat if you like. Dettivo asks once and remembers your acknowledgement in state.toml. Agents pass the acknowledgement on the start request instead.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Rectangle {
            color: Theme.roleHairline
            height: Theme.hairlineWidth
            width: parent.width
        }

        Item {
            height: acknowledgeButton.implicitHeight
            width: parent.width

            Button {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                icon.name: "copy"
                text: qsTr("Copy message")
                onClicked: {
                    if (root.live)
                        root.live.copyDisclosure();
                }
            }

            Button {
                id: notNow
                anchors.right: acknowledgeButton.left
                anchors.rightMargin: Theme.space2
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Not now")
                onClicked: root.dismiss()
            }

            Button {
                id: acknowledgeButton
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                highlighted: true
                text: qsTr("Acknowledge and start")
                onClicked: root.acknowledge()
            }
        }
    }
}
