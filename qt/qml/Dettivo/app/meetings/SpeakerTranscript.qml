pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The transcript tab (meeting-detail.png): one row per segment with the
// time, the speaker's name in its colour and the polished text (or the
// engine's words under Raw); `j` and `k` move the focused row and hand
// its speaker on as the selection, a click on a name opens the rename
// popover for that speaker.
Item {
    id: root

    property var segments: []
    property bool polished: true

    signal speakerPicked(string speakerId)
    // The speaker of the row a key or a tap moved to: the one `r` renames.
    signal rowFocused(string speakerId)

    readonly property int count: root.segments ? root.segments.length : 0
    // The row `j` and `k` moved to; none until a key or a click picks one.
    property int focusRow: -1

    function move(delta) {
        if (root.count === 0)
            return;
        root.focusRow = Math.max(0, Math.min(root.count - 1, root.focusRow + delta));
        list.positionViewAtIndex(root.focusRow, ListView.Contain);
    }

    onFocusRowChanged: {
        if (root.focusRow >= 0 && root.focusRow < root.count)
            root.rowFocused(root.segments[root.focusRow].speakerId || "");
    }

    ListView {
        id: list
        anchors.fill: parent
        clip: true
        model: root.segments
        spacing: Theme.space3

        delegate: TranscriptRow {
            id: row
            required property var modelData
            required property int index
            colorIndex: row.modelData.colorIndex
            gapBefore: row.modelData.gapBefore
            label: row.modelData.speaker
            selected: row.index === root.focusRow
            text: root.polished ? row.modelData.polished : row.modelData.text
            time: row.modelData.time
            width: list.width
            onLabelClicked: root.speakerPicked(row.modelData.speakerId)

            TapHandler {
                onTapped: root.focusRow = row.index
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Meeting transcript")
    }

    StateView {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: Theme.space5
        reason: qsTr("The transcript lands when the finalisation completes; a deleted transcript leaves the notes and the audio.")
        title: qsTr("No transcript.")
        visible: root.count === 0
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Transcript tab")
}
