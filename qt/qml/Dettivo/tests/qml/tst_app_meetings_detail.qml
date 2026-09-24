import Dettivo
import QtQuick
import QtTest

// The live meeting and the meeting detail (fn-34, ADR 0038): the live
// header shows the meters, the elapsed display and a disabled Pause with
// its reason, a transcript row reads provisional in the light weight, the
// notes editor hands its text on, the detail's tabs and toggle answer the
// keys, a speaker click opens the rename popover, the export sheet writes
// Markdown, and delete asks with urgent text before the action runs. The
// fakes are the Fake*.qml files beside this one.
TestCase {
    id: root

    name: "AppMeetingsDetail"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function findByName(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;

            const nested = findByName(child, name);
            if (nested)
                return nested;
        }
        return null;
    }

    function gone(item, name) {
        const found = findByName(item, name);
        return found === null || !found.visible;
    }

    function test_live_header_and_rows() {
        const live = createTemporaryObject(liveC, root, {
            "active": true,
            "recording": true,
            "state": "recording"
        });
        const header = createTemporaryObject(headerC, root, {
            "width": 720,
            "live": live
        });
        waitForRendering(header);
        verify(findByName(header, "Mic meter"));
        verify(findByName(header, "System meter"));
        compare(findByName(header, "Elapsed").Accessible.description, "00:23:41");
        const pause = findByName(header, "Pause");
        verify(pause);
        compare(pause.enabled, false);
        verify(pause.Accessible.description.length > 0);
        const stops = [];
        header.stopRequested.connect(() => stops.push(true));
        mouseClick(findByName(header, "Stop"));
        compare(stops.length, 1);
        // The accepted stop: the meters give way to the acknowledgement,
        // a sweep until the chunks are counted, then the count.
        live.finishing = true;
        compare(header.stateLine, "Stopping · closing the takes");
        verify(findByName(header, "Stopping"));
        verify(findByName(header, "Recording stopped").visible);
        verify(root.gone(header, "Mic meter"));
        compare(findByName(header, "Stop").enabled, false);
        compare(findByName(header, "Finalisation progress").indeterminate, true);
        live.state = "transcribing";
        live.chunksDone = 2;
        live.chunksTotal = 5;
        compare(header.stateLine, "Transcribing · 2 of 5 chunks");
        compare(findByName(header, "Finalisation stage").Accessible.description, "Transcribing · 2 of 5 chunks");
        compare(findByName(header, "Finalisation progress").indeterminate, false);
        compare(findByName(header, "Finalisation progress").value, 0.4);
        live.finishing = false;
        live.state = "recording";
        live.error = "no meeting is recording";
        verify(findByName(header, "Live notice").visible);

        const provisional = createTemporaryObject(rowC, root, {
            "width": 700,
            "time": "21:04",
            "label": "Remote",
            "colorIndex": 1,
            "text": "one more thing",
            "provisional": true,
            "gapBefore": 1500
        });
        waitForRendering(provisional);
        compare(provisional.Accessible.name, "Remote: one more thing");
        compare(provisional.Accessible.description, "provisional");
        compare(provisional.tone, Theme.roleSpeakerColors[1]);
        verify(findByName(provisional, "gap 1.5 s"));
        verify(findByName(provisional, "provisional").visible);
        provisional.provisional = false;
        compare(findByName(provisional, "provisional"), null);
        const labels = [];
        provisional.labelClicked.connect(() => labels.push(true));
        mouseClick(findByName(provisional, "Remote"));
        compare(labels.length, 1);
    }

    function test_notes_editor_hands_its_text_on() {
        const notes = createTemporaryObject(notesC, root, {
            "width": 360,
            "height": 300,
            "text": "## Decisions",
            "saveState": "Saved"
        });
        waitForRendering(notes);
        verify(findByName(notes, "Markdown · Saved"));
        const edits = [];
        notes.edited.connect(t => edits.push(t));
        notes.takeFocus();
        compare(notes.typing, true);
        keyClick(Qt.Key_X);
        compare(edits[edits.length - 1], "## Decisionsx");
    }

    function test_detail_tabs_toggle_rename_export_and_delete() {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(historyActionsC, root);
        const meetingsActions = createTemporaryObject(meetingsActionsC, root);
        const pane = createTemporaryObject(detailPaneC, root, {
            "width": 1080,
            "height": 800,
            "meetingId": "m1",
            "detail": detail,
            "actions": actions,
            "meetingsActions": meetingsActions
        });
        waitForRendering(pane);
        verify(findByName(pane, "Meeting header"));
        verify(findByName(pane, "Rename Mara"));
        verify(findByName(pane, "Analysis rail"));
        verify(findByName(pane, "exports: md · txt · srt · vtt · json"));
        compare(findByName(pane, "Re-run").enabled, false);
        compare(pane.polished, true);
        pane.togglePolished();
        compare(pane.polished, false);
        pane.pickTab(2);
        verify(findByName(pane, "Analyse again"));
        pane.pickTab(1);
        compare(pane.typing, false);
        pane.pickTab(0);
        pane.move(1);
        pane.move(1);

        const overlay = root.Window.window.contentItem;
        mouseClick(findByName(pane, "Rename Mara"));
        tryVerify(() => {
            return findByName(overlay, "Rename speaker") !== null;
        });
        compare(meetingsActions.suggested.length, 1);
        mouseClick(findByName(overlay, "Use Tobias"));
        compare(meetingsActions.renames[0].name, "Tobias");
        compare(meetingsActions.renames[0].speakerId, "speaker_00");
        tryVerify(() => {
            return root.gone(overlay, "Rename speaker");
        });
        compare(detail.renamed[0], "speaker_00:Tobias");

        pane.exportSheet();
        tryVerify(() => {
            return findByName(overlay, "Meeting export sheet") !== null;
        });
        mouseClick(findByName(overlay, "Write file"));
        compare(actions.exports[0].format, "md");
        compare(actions.exports[0].raw, false);
        mouseClick(findByName(overlay, "Close"));
        tryVerify(() => {
            return root.gone(overlay, "Meeting export sheet");
        });

        mouseClick(findByName(pane, "Delete meeting"));
        tryVerify(() => {
            return findByName(overlay, "Delete meeting for good") !== null;
        });
        mouseClick(findByName(overlay, "Delete meeting for good"));
        compare(meetingsActions.removed[0], "m1");
    }

    // `j` and `k` move the transcript row and `r` renames the speaker of
    // that row, not the first speaker or the last one clicked; the
    // talk-time bar follows the same selection.
    function test_keyboard_rename_targets_the_focused_rows_speaker() {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(historyActionsC, root);
        const meetingsActions = createTemporaryObject(meetingsActionsC, root);
        const pane = createTemporaryObject(detailPaneC, root, {
            "width": 1080,
            "height": 800,
            "meetingId": "m1",
            "detail": detail,
            "actions": actions,
            "meetingsActions": meetingsActions
        });
        waitForRendering(pane);
        const overlay = root.Window.window.contentItem;
        compare(pane.speakerIndex, 0);
        pane.move(1);
        pane.move(1);
        compare(pane.speakerIndex, 1);
        pane.renameSpeaker();
        tryVerify(() => {
            return findByName(overlay, "Rename speaker") !== null;
        });
        mouseClick(findByName(overlay, "Use Tobias"));
        compare(meetingsActions.renames[0].speakerId, "speaker_00");
        tryVerify(() => {
            return root.gone(overlay, "Rename speaker");
        });
        pane.move(-1);
        compare(pane.speakerIndex, 0);
        pane.renameSpeaker();
        tryVerify(() => {
            return findByName(overlay, "Rename speaker") !== null;
        });
        mouseClick(findByName(overlay, "Use Tobias"));
        compare(meetingsActions.renames[1].speakerId, "you");
        tryVerify(() => {
            return root.gone(overlay, "Rename speaker");
        });
    }

    Component {
        id: liveC
        FakeMeetingLive {}
    }

    Component {
        id: detailC
        FakeMeetingDetail {}
    }

    Component {
        id: historyActionsC
        FakeHistoryActions {}
    }

    Component {
        id: meetingsActionsC
        FakeMeetingsActions {}
    }

    Component {
        id: headerC
        LiveHeader {}
    }

    Component {
        id: rowC
        TranscriptRow {}
    }

    Component {
        id: notesC
        NotesEditor {}
    }

    Component {
        id: detailPaneC
        MeetingDetail {}
    }
}
