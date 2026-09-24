pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Export a meeting (designed inline against the export sheet): the five
// formats, the engine's words instead of the polished segments, where
// the file goes (the portal's save dialog, or the QA export directory),
// one primary action. The outcome, a path or the daemon's refusal, lands
// in the sheet.
Popup {
    id: root

    property var actions: null
    property string meetingId: ""
    property string notes: ""
    property string outcome: ""
    property bool refused: false
    property int formatIndex: 0

    readonly property string exportDir: root.actions ? root.actions.exportDir : ""
    readonly property var formats: [
        {
            "id": "md",
            "label": qsTr("Markdown, with the notes and the analysis")
        },
        {
            "id": "txt",
            "label": qsTr("Text")
        },
        {
            "id": "srt",
            "label": qsTr("SubRip captions")
        },
        {
            "id": "vtt",
            "label": qsTr("WebVTT captions")
        },
        {
            "id": "json",
            "label": qsTr("JSON")
        }
    ]

    function openFor(id, currentNotes) {
        root.meetingId = id;
        root.notes = currentNotes;
        root.outcome = "";
        root.refused = false;
        root.open();
    }

    function chosenFormat() {
        return root.formats[root.formatIndex].id;
    }

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 8

    Connections {
        target: root.actions
        function onExported(path, bytes) {
            root.refused = false;
            root.outcome = qsTr("Wrote %1 (%2 bytes).").arg(path).arg(bytes);
        }
        function onFailed(action, reason) {
            if (action === "export") {
                root.refused = true;
                root.outcome = reason;
            }
        }
    }

    ButtonGroup {
        id: formatGroup
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space4

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Meeting export sheet")

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeHeadingSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Export meeting")
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("Export meeting")
        }

        SectionLabel {
            leftPadding: 0
            text: qsTr("Format")
        }

        Column {
            spacing: 0

            Repeater {
                model: root.formats

                delegate: RadioButton {
                    id: format
                    required property var modelData
                    required property int index
                    ButtonGroup.group: formatGroup
                    checked: format.index === 0
                    text: format.modelData.label
                    onCheckedChanged: {
                        if (format.checked)
                            root.formatIndex = format.index;
                    }
                }
            }
        }

        CheckBox {
            id: rawBox
            width: parent.width
            text: qsTr("The engine's words instead of the polished segments")
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.exportDir.length > 0 ? qsTr("Written into %1.").arg(root.exportDir) : qsTr("The desktop's save dialog asks where the file goes.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Export destination")
        }

        Text {
            color: root.refused ? Theme.roleUrgent : Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.outcome
            visible: root.outcome.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Export outcome")
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.space2

            Button {
                text: qsTr("Close")
                onClicked: root.close()
            }

            Button {
                enabled: root.actions !== null && !root.actions.busy
                highlighted: true
                text: qsTr("Write file")
                onClicked: {
                    root.outcome = "";
                    root.refused = false;
                    root.actions.exportMeeting(root.meetingId, root.chosenFormat(), rawBox.checked, root.notes);
                }
            }
        }
    }
}
