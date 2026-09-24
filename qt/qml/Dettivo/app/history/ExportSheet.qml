pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Export (designed inline against the settings pattern): the format, the
// scope (this dictation or everything), where the file goes (the portal's
// save dialog, or the QA export directory), one primary action. The
// outcome, a path or the daemon's refusal, lands in the sheet.
Popup {
    id: root

    property var actions: null
    property string itemId: ""
    property string outcome: ""
    property bool refused: false
    property int formatIndex: 0

    readonly property string exportDir: root.actions ? root.actions.exportDir : ""
    readonly property var formats: [
        {
            "id": "json",
            "label": qsTr("JSON")
        },
        {
            "id": "md",
            "label": qsTr("Markdown")
        },
        {
            "id": "txt",
            "label": qsTr("Text")
        },
        {
            "id": "zip",
            "label": qsTr("Archive with audio")
        }
    ]

    function openFor(id) {
        root.itemId = id;
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

    ButtonGroup {
        id: scopeGroup
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space4

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Export sheet")

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeHeadingSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Export")
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("Export heading")
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

        SectionLabel {
            leftPadding: 0
            text: qsTr("Scope")
        }

        Column {
            spacing: 0

            RadioButton {
                id: scopeItem
                ButtonGroup.group: scopeGroup
                checked: true
                text: qsTr("This dictation")
            }

            RadioButton {
                id: scopeAll
                ButtonGroup.group: scopeGroup
                text: qsTr("Everything")
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.exportDir.length > 0 ? qsTr("Written into %1.").arg(root.exportDir) : qsTr("The desktop's save dialog asks where the file goes.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Destination")
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
                    root.actions.exportItems(root.itemId, root.chosenFormat(), scopeAll.checked ? "all" : "item");
                }
            }
        }
    }
}
