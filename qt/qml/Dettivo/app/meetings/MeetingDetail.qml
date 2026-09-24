pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The meeting detail (meeting-detail.png): the title (the rename control,
// `t`) with Re-run and Export, the facts line, the processing strip
// while the speaker pass or the analysis is still to come (ADR 0061),
// the talk-time bar per speaker, the Transcript, Notes and Analysis tabs
// with the Raw and Polished toggle, the analysis rail with the summary,
// the decisions and the action items over the notes, export and audio
// facts, and delete as urgent text in the footer. Without a meeting the
// designed state says what to pick.
Item {
    id: root

    property string meetingId: ""
    property var router: null
    property var detail: null
    property var actions: null
    property var meetingsActions: null
    property string qaState: ""

    readonly property bool hasItem: root.meetingId.length > 0
    readonly property bool loaded: root.detail ? root.detail.loaded : false
    readonly property string failure: root.detail ? root.detail.error : ""
    readonly property bool typing: (tabRow.currentIndex === 1 && notesTab.typing) || header.editing
    property bool polished: true
    property int speakerIndex: 0
    property string actionNote: ""

    function pickTab(index) {
        tabRow.currentIndex = Math.max(0, Math.min(2, index));
    }

    function togglePolished() {
        root.polished = !root.polished;
    }

    function move(delta) {
        transcriptTab.move(delta);
    }

    function renameTitle() {
        if (root.loaded)
            header.beginRename();
    }

    function renameSpeaker() {
        const speaker = root.detail ? root.detail.speakerAt(root.speakerIndex) : ({});
        if (speaker && speaker.speakerId)
            rename.openFor(root.meetingId, speaker.speakerId, speaker.name, bar.anchorFor(root.speakerIndex));
    }

    function selectSpeaker(speakerId) {
        const speakers = root.detail ? root.detail.speakers : [];
        const i = speakers.findIndex(s => s.speakerId === speakerId);
        if (i >= 0)
            root.speakerIndex = i;
    }

    function exportSheet() {
        if (root.loaded)
            sheet.openFor(root.meetingId, root.detail ? root.detail.notes : "");
    }

    function confirmDelete() {
        if (root.loaded)
            deleteConfirm.openFor(root.meetingId);
    }

    function dismiss() {
        if (header.editing) {
            header.cancelRename();
            return true;
        }
        for (const popup of [rename, sheet, deleteConfirm]) {
            if (popup.visible) {
                popup.close();
                return true;
            }
        }
        return false;
    }

    onMeetingIdChanged: root.actionNote = ""

    readonly property var tabNames: ["transcript", "notes", "analysis"]

    Component.onCompleted: {
        if (root.qaState === "detail-notes")
            tabRow.currentIndex = 1;
        else if (root.qaState === "detail-analysis")
            tabRow.currentIndex = 2;
        else if (root.detail && root.detail.lastTab !== undefined)
            tabRow.currentIndex = Math.max(0, root.tabNames.indexOf(root.detail.lastTab));
    }

    DetailFollowers {
        page: root
    }

    RouteScaffold {
        anchors.fill: parent
        stateReason: root.hasItem ? (root.failure.length > 0 ? root.failure : qsTr("Reading the meeting.")) : qsTr("Pick a meeting in Meetings to see its transcript, notes and analysis.")
        stateTitle: root.hasItem ? (root.failure.length > 0 ? qsTr("Meeting unavailable.") : qsTr("Meeting selected.")) : qsTr("No meeting selected.")
        subtitle: qsTr("Transcript, notes and analysis after the stop.")
        title: qsTr("Meeting")
        urgent: root.failure.length > 0
        visible: !root.loaded
    }

    Item {
        id: main
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: railPane.left
        anchors.top: parent.top
        visible: root.loaded

        DetailHeader {
            id: header
            actions: root.actions
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            detail: root.detail
            meetingId: root.meetingId
            meetingsActions: root.meetingsActions
            note: root.actionNote
            onExportRequested: root.exportSheet()
        }

        ProcessingStrip {
            id: processing
            anchors.left: parent.left
            anchors.leftMargin: Theme.pagePaddingX
            anchors.right: parent.right
            anchors.rightMargin: Theme.pagePaddingX
            anchors.top: header.bottom
            detail: root.detail
        }

        TalkTimeBar {
            id: bar
            anchors.left: parent.left
            anchors.leftMargin: Theme.pagePaddingX
            anchors.right: parent.right
            anchors.rightMargin: Theme.pagePaddingX
            anchors.top: processing.bottom
            anchors.topMargin: Theme.space3
            currentIndex: root.speakerIndex
            hint: root.detail && root.detail.diarizationStatus !== "ready" ? root.detail.diarizationLine : qsTr("rename a speaker: click the name")
            speakers: root.detail ? root.detail.speakers : []
            onSpeakerClicked: (index, anchorItem) => {
                root.speakerIndex = index;
                root.renameSpeaker();
            }
        }

        DetailTabs {
            id: tabRow
            anchors.left: parent.left
            anchors.leftMargin: Theme.pagePaddingX
            anchors.right: parent.right
            anchors.rightMargin: Theme.pagePaddingX
            anchors.top: bar.bottom
            anchors.topMargin: Theme.space4
            polished: root.polished
            onPolishedChanged: root.polished = tabRow.polished
            onCurrentIndexChanged: {
                if (root.detail && root.detail.lastTab !== undefined)
                    root.detail.lastTab = root.tabNames[tabRow.currentIndex];
            }
        }

        StackLayoutLite {
            id: pagesItem
            anchors.bottom: footer.top
            anchors.left: parent.left
            anchors.leftMargin: Theme.pagePaddingX
            anchors.right: parent.right
            anchors.rightMargin: Theme.pagePaddingX
            anchors.top: tabRow.bottom
            anchors.topMargin: Theme.space4
            currentIndex: tabRow.currentIndex

            SpeakerTranscript {
                id: transcriptTab
                anchors.fill: parent
                polished: root.polished
                segments: root.detail ? root.detail.segments : []
                onRowFocused: speakerId => root.selectSpeaker(speakerId)
                onSpeakerPicked: speakerId => {
                    root.selectSpeaker(speakerId);
                    root.renameSpeaker();
                }
            }

            NotesEditor {
                id: notesTab
                anchors.fill: parent
                saveState: root.detail ? root.detail.notesState : ""
                text: root.detail ? root.detail.notes : ""
                onEdited: markdown => {
                    if (root.detail)
                        root.detail.setNotes(markdown);
                }
            }

            AnalysisView {
                id: analysisTab
                actions: root.meetingsActions
                anchors.fill: parent
                detail: root.detail
                meetingId: root.meetingId
            }
        }

        MeetingsFooter {
            id: footer
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            hairline: true
            // Noun labels: the full line fits beside the delete control at
            // the default window (fn-62); the hint sheet carries the verbs.
            hints: qsTr("j / k move · 1 2 3 tabs · p polished · r speaker · t title · e export · d delete")
            trailingReserve: deleteAction.width

            TextAction {
                id: deleteAction
                accessibleName: qsTr("Delete meeting")
                anchors.right: parent.right
                anchors.rightMargin: Theme.pagePaddingX
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("delete meeting")
                onTriggered: root.confirmDelete()
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.right: railPane.left
        anchors.top: parent.top
        color: Theme.roleHairline
        visible: root.loaded
        width: Theme.hairlineWidth
    }

    AnalysisRail {
        id: railPane
        actions: root.meetingsActions
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.top: parent.top
        detail: root.detail
        meetingId: root.meetingId
        visible: root.loaded
        width: Theme.rightRailWidth + Theme.space8
    }

    SpeakerRenamePopover {
        id: rename
        actions: root.meetingsActions
    }

    MeetingExportSheet {
        id: sheet
        actions: root.actions
        anchors.centerIn: parent
    }

    MeetingDeleteConfirm {
        id: deleteConfirm
        actions: root.meetingsActions
        anchors.centerIn: parent
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meeting detail")
}
