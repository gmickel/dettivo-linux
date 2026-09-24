pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Rename a speaker (designed inline against the pattern): a popover under
// the speaker's name with the name field, the remembered names from
// `meetings.speakers.suggest` as a list, Enter applies, Escape cancels,
// an empty name restores the label. The daemon's refusal (a name over 64
// characters, a running pass) lands under the field.
Popup {
    id: root

    property var actions: null
    property string meetingId: ""
    property string speakerId: ""
    property string currentName: ""
    property string error: ""

    readonly property var suggestions: root.actions ? root.actions.suggestions : []

    function openFor(meeting, speaker, name, anchorItem) {
        root.meetingId = meeting;
        root.speakerId = speaker;
        root.currentName = name;
        root.error = "";
        field.text = name;
        if (root.actions)
            root.actions.suggest("");
        // The overlay may not be sized yet when a render opens the popover
        // at once; the clamp applies only where a size is known.
        const tall = root.contentItem.implicitHeight + root.topPadding + root.bottomPadding;
        const host = root.parent;
        const wide = host && host.width > 0 ? host.width : Number.MAX_VALUE;
        const high = host && host.height > 0 ? host.height : Number.MAX_VALUE;
        if (anchorItem) {
            const at = anchorItem.mapToItem(null, 0, anchorItem.height);
            root.x = Math.max(Theme.space2, Math.min(at.x, wide - root.width - Theme.space2));
            root.y = Math.max(Theme.space2, Math.min(at.y + Theme.space2, high - tall - Theme.space2));
        } else if (host) {
            root.x = (host.width - root.width) / 2;
            root.y = (host.height - tall) / 2;
        }
        root.open();
        field.forceActiveFocus();
        field.selectAll();
    }

    function apply(name) {
        root.error = "";
        if (root.actions)
            root.actions.rename(root.meetingId, root.speakerId, name);
    }

    modal: true
    parent: Overlay.overlay
    padding: Theme.space4
    width: Theme.space8 * 6

    Connections {
        target: root.actions
        function onRenamed(meetingId, speakerId, name, segmentsUpdated) {
            if (meetingId === root.meetingId && speakerId === root.speakerId)
                root.close();
        }
        function onFailed(action, reason) {
            if (action === "rename")
                root.error = reason;
        }
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space3

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Rename speaker")

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Name for %1").arg(root.currentName)
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("Rename speaker heading")
        }

        TextField {
            id: field
            placeholderText: qsTr("empty restores the label")
            width: parent.width
            onAccepted: root.apply(field.text)
            onTextEdited: {
                if (root.actions)
                    root.actions.suggest(field.text);
            }
            Accessible.role: Accessible.EditableText
            Accessible.name: qsTr("Speaker name")
        }

        Column {
            spacing: 0
            visible: root.suggestions.length > 0
            width: parent.width

            SectionLabel {
                leftPadding: 0
                text: qsTr("Remembered")
            }

            Repeater {
                model: root.suggestions

                delegate: Text {
                    id: suggestion
                    required property var modelData
                    color: Theme.roleMutedText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    height: Theme.controlHeight - Theme.space2
                    text: suggestion.modelData.name
                    textFormat: Text.PlainText
                    verticalAlignment: Text.AlignVCenter
                    width: parent.width

                    TapHandler {
                        onTapped: root.apply(suggestion.modelData.name)
                    }

                    activeFocusOnTab: true
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                            root.apply(suggestion.modelData.name);
                            event.accepted = true;
                        }
                    }

                    FocusRing {}

                    Accessible.role: Accessible.Button
                    Accessible.name: qsTr("Use %1").arg(suggestion.modelData.name)
                    Accessible.onPressAction: root.apply(suggestion.modelData.name)
                }
            }
        }

        Text {
            color: Theme.roleUrgent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.error
            visible: root.error.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Rename refused")
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.space2

            Button {
                text: qsTr("Cancel")
                onClicked: root.close()
            }

            Button {
                enabled: root.actions !== null && !root.actions.busy
                highlighted: true
                text: qsTr("Apply name")
                onClicked: root.apply(field.text)
            }
        }
    }
}
