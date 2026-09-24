pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Import audio (import-and-disclosure.png): the file with its length and
// size, the engine, the language, diarize after
// transcription, the analysis, the sentence on the chunking and the
// estimate, and one primary Import that maps onto `transcripts.import`.
// The file is named by its path (the field, or the desktop's file
// chooser); a rejected file names the daemon's reason.
Popup {
    id: root

    property var actions: null
    property string error: ""
    property var facts: ({})
    property bool editing: true

    readonly property var providers: root.actions ? root.actions.providers : []
    readonly property bool fileOk: root.facts && root.facts.ok === true
    readonly property string estimate: {
        if (!root.fileOk)
            return "";
        const ms = root.facts.durationMs;
        if (ms === undefined || ms < 0)
            return qsTr("Chunked at 5 minutes with 2 s overlap. The original file is copied into the meeting folder.");
        const minutes = Math.max(1, Math.round(ms / 60000 / 8));
        return qsTr("Chunked at 5 minutes with 2 s overlap. About %1 on this engine. The original file is copied into the meeting folder.").arg(minutes === 1 ? qsTr("a minute") : qsTr("%1 minutes").arg(minutes));
    }

    function openFor(path) {
        root.error = "";
        pathField.text = path;
        root.editing = true;
        root.probe();
        root.open();
        pathField.forceActiveFocus();
    }

    // A render opens the dialog over a file the artboard names, without
    // a file on disk.
    function openSample() {
        root.error = "";
        pathField.text = "interview-raw.m4a";
        root.facts = {
            "ok": true,
            "name": "interview-raw.m4a",
            "durationMs": 64 * 60000,
            "durationText": qsTr("64 min"),
            "sizeText": "58 MB"
        };
        root.editing = false;
        root.open();
    }

    function probe() {
        if (root.editing)
            root.facts = root.actions ? root.actions.probeFile(pathField.text) : ({});
    }

    function start() {
        root.error = "";
        const provider = root.providers[engine.currentIndex];
        const language = languageBox.currentIndex === 0 ? "auto" : ["en", "de", "fr", "es", "it"][languageBox.currentIndex - 1];
        if (root.actions)
            root.actions.importFile(pathField.text, provider ? provider.id : "", "", language, diarizeBox.checked, analyseBox.checked);
    }

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 11

    Connections {
        target: root.actions
        function onImportStarted(meetingId, jobId) {
            root.close();
        }
        function onFailed(action, reason) {
            if (action === "import")
                root.error = reason;
        }
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space3

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Import dialog")

        Item {
            height: heading.implicitHeight
            width: parent.width

            Text {
                id: heading
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeHeadingSize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Import audio")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("Import audio")
            }

            SectionLabel {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                rightPadding: 0
                text: qsTr("Meeting")
            }
        }

        Rectangle {
            color: Theme.roleHairline
            height: Theme.hairlineWidth
            width: parent.width
        }

        ImportField {
            label: qsTr("file")
            width: parent.width

            TextField {
                id: pathField
                placeholderText: qsTr("~/Recordings/call.m4a")
                visible: root.editing
                width: parent.width
                onTextChanged: root.probe()
                onAccepted: {
                    if (root.fileOk)
                        root.editing = false;
                }
                Accessible.role: Accessible.EditableText
                Accessible.name: qsTr("Import file")
            }

            Text {
                color: Theme.roleText
                elide: Text.ElideRight
                font.family: Theme.fontFamily
                font.features: Theme.typeTabularNumerals
                font.pixelSize: Theme.typeBodySize
                text: root.fileOk ? qsTr("%1 · %2 · %3").arg(root.facts.name).arg(root.facts.durationText).arg(root.facts.sizeText) : ""
                visible: !root.editing
                width: parent.width

                TapHandler {
                    onTapped: {
                        root.editing = true;
                        pathField.forceActiveFocus();
                    }
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        root.editing = true;
                        pathField.forceActiveFocus();
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("File facts")
                Accessible.description: text
                Accessible.onPressAction: {
                    root.editing = true;
                    pathField.forceActiveFocus();
                }
            }
        }

        Text {
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            leftPadding: Theme.space8 * 2 + Theme.space6
            text: root.fileOk ? qsTr("%1 · %2").arg(root.facts.durationText).arg(root.facts.sizeText) : (root.facts && root.facts.reason ? root.facts.reason : "")
            visible: root.editing && text.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("File check")
        }

        ImportField {
            label: qsTr("engine")
            width: parent.width

            ComboBox {
                id: engine
                model: root.providers
                textRole: "label"
                width: parent.width
                Accessible.role: Accessible.ComboBox
                Accessible.name: qsTr("Import engine")
            }
        }

        ImportField {
            label: qsTr("language")
            width: parent.width

            ComboBox {
                id: languageBox
                model: [qsTr("Auto-detect"), qsTr("English"), qsTr("German"), qsTr("French"), qsTr("Spanish"), qsTr("Italian")]
                width: parent.width
                Accessible.role: Accessible.ComboBox
                Accessible.name: qsTr("Import language")
            }
        }

        ImportField {
            label: qsTr("speakers")
            width: parent.width

            CheckBox {
                id: diarizeBox
                checked: true
                text: qsTr("diarize after transcription")
            }
        }

        ImportField {
            label: qsTr("analysis")
            width: parent.width

            CheckBox {
                id: analyseBox
                checked: true
                text: qsTr("summary, decisions, action items")
            }
        }

        Text {
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.estimate.length > 0 ? root.estimate : qsTr("Chunked at 5 minutes with 2 s overlap. The original file is copied into the meeting folder.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Import estimate")
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
            Accessible.name: qsTr("Import refused: %1").arg(root.error)
        }

        Rectangle {
            color: Theme.roleHairline
            height: Theme.hairlineWidth
            width: parent.width
        }

        ImportFooter {
            ready: root.fileOk && root.actions !== null && !root.actions.busy
            width: parent.width
            onAccepted: root.start()
            onCancelled: {
                if (root.actions && root.actions.cancelImport)
                    root.actions.cancelImport();
                root.close();
            }
        }
    }
}
