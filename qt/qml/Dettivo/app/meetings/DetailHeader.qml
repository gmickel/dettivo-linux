import QtQuick
import QtQuick.Controls
import Dettivo

// The head of the meeting detail (meeting-detail.png): the title with
// Re-run (disabled: a meeting re-run stays reserved in the contract, the
// reason is its description) and Export on the right, the facts line
// under it, and the outcome of the last action where it belongs. The
// title is the rename control (ADR 0061): a click, Return or `t` opens
// it as a field with the current title selected, Return saves through
// `meetings.rename`, Escape restores the saved name, and a refusal reads
// under the field with the edit kept.
Item {
    id: root

    property var detail: null
    property var actions: null
    property var meetingsActions: null
    property string meetingId: ""
    property string note: ""
    property string renameError: ""

    readonly property bool editing: editor.visible
    readonly property string titleText: root.detail && root.detail.title.length > 0 ? root.detail.title : qsTr("Meeting")

    signal exportRequested

    function beginRename() {
        if (!root.detail || root.editing)
            return;
        root.renameError = "";
        editor.text = root.detail.title;
        editor.visible = true;
        editor.forceActiveFocus();
        editor.selectAll();
    }

    function commitRename() {
        if (!root.editing)
            return;
        const next = editor.text.trim();
        if (next.length === 0) {
            root.renameError = qsTr("A meeting needs a title.");
            return;
        }
        if (next === (root.detail ? root.detail.title : "")) {
            root.cancelRename(false);
            return;
        }
        root.renameError = "";
        if (root.meetingsActions)
            root.meetingsActions.retitle(root.meetingId, next);
    }

    function cancelRename(restoreFocus) {
        root.renameError = "";
        editor.deselect();
        editor.visible = false;
        if (restoreFocus === false)
            root.forceActiveFocus();
        else
            title.forceActiveFocus();
    }

    implicitHeight: Theme.pagePaddingY + Math.max(title.implicitHeight, editor.visible ? editor.implicitHeight : 0) + (renameNote.visible ? Theme.space1 + renameNote.implicitHeight : 0) + Theme.space2 + facts.implicitHeight + (noteText.visible ? Theme.space2 + noteText.implicitHeight : 0) + Theme.space3

    Connections {
        target: root.meetingsActions
        function onRetitled(meetingId, name) {
            if (meetingId !== root.meetingId)
                return;
            if (root.detail && root.detail.applyTitle)
                root.detail.applyTitle(name);
            root.cancelRename(false);
        }
        function onFailed(action, reason) {
            if (action === "retitle" && root.editing)
                root.renameError = reason;
        }
    }

    Text {
        id: title
        activeFocusOnTab: true
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: buttons.left
        anchors.rightMargin: Theme.space4
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY
        color: Theme.roleText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
        font.pixelSize: Theme.typeTitleSize
        font.weight: Theme.typeEmphasisWeight
        text: root.titleText
        textFormat: Text.PlainText
        visible: !editor.visible

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.beginRename();
                event.accepted = true;
            }
        }

        TapHandler {
            onTapped: root.beginRename()
        }

        HoverHandler {
            cursorShape: Qt.IBeamCursor
        }

        FocusRing {}

        Accessible.role: Accessible.Button
        Accessible.name: qsTr("Rename meeting")
        Accessible.description: root.titleText
        Accessible.onPressAction: root.beginRename()
    }

    TextField {
        id: editor
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX - Theme.controlPaddingX
        anchors.right: buttons.left
        anchors.rightMargin: Theme.space4
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY - (implicitHeight - title.implicitHeight) / 2
        font.family: Theme.fontFamily
        font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
        font.pixelSize: Theme.typeTitleSize
        font.weight: Theme.typeEmphasisWeight
        maximumLength: 200
        placeholderText: qsTr("Meeting title")
        visible: false
        onAccepted: root.commitRename()
        Keys.onEscapePressed: root.cancelRename()
        Accessible.role: Accessible.EditableText
        Accessible.name: qsTr("Meeting title")
    }

    Text {
        id: renameNote
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: editor.visible ? editor.bottom : title.bottom
        anchors.topMargin: Theme.space1
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.renameError
        visible: root.renameError.length > 0
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Rename refused")
    }

    Row {
        id: buttons
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.verticalCenter: title.verticalCenter
        spacing: Theme.space2

        Button {
            enabled: false
            icon.name: "rerun"
            text: qsTr("Re-run")
            Accessible.description: qsTr("A meeting re-run is reserved in the contract; import the audio again to transcribe it with another engine.")
        }

        Button {
            enabled: root.actions !== null && !root.actions.busy
            icon.name: "export"
            text: qsTr("Export")
            onClicked: root.exportRequested()
        }
    }

    Text {
        id: facts
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: renameNote.visible ? renameNote.bottom : (editor.visible ? editor.bottom : title.bottom)
        anchors.topMargin: Theme.space2
        color: Theme.roleMutedText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.detail ? (root.detail.partial ? root.detail.factsLine + " · " + root.detail.partialLine : root.detail.factsLine) : ""
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Facts")
        Accessible.description: text
    }

    Text {
        id: noteText
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: facts.bottom
        anchors.topMargin: Theme.space2
        color: root.note.startsWith(qsTr("Wrote")) ? Theme.roleMutedText : Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.note
        visible: root.note.length > 0
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Action outcome")
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meeting header")
}
