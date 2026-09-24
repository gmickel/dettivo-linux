import Dettivo
import QtQuick
import QtTest

// What fn-63 added to the meeting detail (ADR 0061): the title renames
// the meeting inline (a click opens the field, Return saves through the
// actions, Escape keeps the saved name, a refusal keeps the edit, the
// letters never reach the page's keys), and the processing strip reads
// the stages while a pass is still to come or one failed. The fakes are
// the Fake*.qml files beside this one.
TestCase {
    id: root

    name: "MeetingDetailProcessing"
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

    // The title renames the meeting: a click opens the field with the
    // title selected, Return saves through the actions and relabels the
    // header, Escape keeps the saved name, a refusal keeps the edit, and
    // the letters typed into the field never reach the page's keys.
    function test_title_renames_inline() {
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
        const control = findByName(pane, "Rename meeting");
        verify(control);
        compare(control.Accessible.description, "Dettivo Linux kickoff");
        verify(root.gone(pane, "Meeting title"));
        mouseClick(control);
        const field = findByName(pane, "Meeting title");
        verify(field.visible);
        verify(field.activeFocus);
        compare(field.selectedText, "Dettivo Linux kickoff");
        compare(pane.typing, true);
        keyClick(Qt.Key_Escape);
        tryVerify(() => root.gone(pane, "Meeting title"));
        compare(meetingsActions.retitles.length, 0);
        compare(detail.title, "Dettivo Linux kickoff");
        compare(pane.typing, false);

        pane.renameTitle();
        tryVerify(() => findByName(pane, "Meeting title").visible);
        keyClick(Qt.Key_End);
        keyClick(Qt.Key_Comma);
        keyClick(Qt.Key_Space);
        keyClick(Qt.Key_2);
        keyClick(Qt.Key_Return);
        compare(meetingsActions.retitles[0], "Dettivo Linux kickoff, 2");
        compare(detail.titles[0], "Dettivo Linux kickoff, 2");
        tryVerify(() => root.gone(pane, "Meeting title"));
        compare(findByName(pane, "Rename meeting").Accessible.description, "Dettivo Linux kickoff, 2");
        verify(!control.activeFocus, "saving leaves the title without a focus highlight");
        compare(field.selectedText, "");

        meetingsActions.refuseTitle = "title is 201 characters; the limit is 200";
        pane.renameTitle();
        tryVerify(() => findByName(pane, "Meeting title").visible);
        keyClick(Qt.Key_End);
        keyClick(Qt.Key_X);
        keyClick(Qt.Key_Return);
        verify(findByName(pane, "Meeting title").visible);
        compare(findByName(pane, "Rename refused").text, "title is 201 characters; the limit is 200");
        compare(findByName(pane, "Meeting title").text, "Dettivo Linux kickoff, 2x");
        compare(pane.dismiss(), true);
        tryVerify(() => root.gone(pane, "Meeting title"));
    }

    // The strip says which pass is still to come, keeps the urgent tone
    // after a failure, the highlight tone when a wanted pass had no
    // model, and stays muted with the facts once every stage settled.
    function test_processing_strip_reads_the_stages() {
        const detail = createTemporaryObject(detailC, root);
        const strip = createTemporaryObject(stripC, root, {
            "width": 700,
            "detail": detail
        });
        waitForRendering(strip);
        verify(strip.visible);
        compare(strip.tone, Theme.roleMutedText);
        compare(findByName(strip, "Processing").Accessible.description, "Processing complete");
        compare(findByName(strip, "Speakers state").Accessible.description, "done · 2 speakers");
        verify(root.gone(strip, "Processing progress"));
        detail.stages = [
            {
                "key": "transcript",
                "label": "Transcript",
                "state": "done",
                "detail": "948 segments",
                "progress": -1
            },
            {
                "key": "speakers",
                "label": "Speakers",
                "state": "running",
                "detail": "identifying who spoke",
                "progress": 0.25
            },
            {
                "key": "analysis",
                "label": "Analysis",
                "state": "queued",
                "detail": "after the speakers",
                "progress": -1
            }
        ];
        detail.processingState = "running";
        detail.processingLine = "Transcript ready · speakers running";
        waitForRendering(strip);
        verify(strip.visible);
        compare(findByName(strip, "Processing").Accessible.description, "Transcript ready · speakers running");
        verify(findByName(strip, "Speakers: running"));
        compare(findByName(strip, "Analysis state").Accessible.description, "queued · after the speakers");
        compare(findByName(strip, "Processing progress").value, 0.25);
        compare(strip.tone, Theme.roleAccent);
        const settled = detail.stages.slice();
        settled[1] = Object.assign({}, settled[1], {
            "state": "done"
        });
        settled[2] = Object.assign({}, settled[2], {
            "state": "failed"
        });
        detail.stages = settled;
        detail.processingState = "failed";
        detail.processingLine = "Processing finished · analysis failed";
        waitForRendering(strip);
        verify(strip.visible);
        compare(strip.tone, Theme.roleUrgent);
        verify(root.gone(strip, "Processing progress"));
        const incomplete = settled.slice();
        incomplete[1] = Object.assign({}, incomplete[1], {
            "state": "unavailable",
            "detail": "no speaker model downloaded"
        });
        incomplete[2] = Object.assign({}, incomplete[2], {
            "state": "skipped",
            "detail": "not run"
        });
        detail.stages = incomplete;
        detail.processingState = "incomplete";
        detail.processingLine = "Processing incomplete · speakers skipped, no speaker model downloaded";
        waitForRendering(strip);
        verify(strip.visible);
        compare(strip.tone, Theme.roleHighlight);
        verify(findByName(strip, "Speakers: skipped"));
        compare(findByName(strip, "Speakers state").Accessible.description, "skipped · no speaker model downloaded");
        compare(findByName(strip, "Analysis state").Accessible.description, "not run · not run");
        detail.stages = detail.stages.map(s => Object.assign({}, s, {
                "state": "done",
                "detail": "3 speakers · 154 segments unassigned"
            }));
        detail.processingState = "done";
        detail.processingLine = "Processing complete";
        waitForRendering(strip);
        verify(strip.visible);
        compare(strip.tone, Theme.roleMutedText);
        // The unassigned count stays on screen (fn-62): the detail takes
        // the room its row has instead of eliding at a fixed cap.
        const speakers = findByName(strip, "Speakers state");
        compare(speakers.Accessible.description, "done · 3 speakers · 154 segments unassigned");
        compare(speakers.truncated, false);
        compare(speakers.width, speakers.implicitWidth);
    }

    // The footer's keys never run under the delete control (fn-62): at
    // the default window the whole line shows on one line, at the
    // narrowest window it wraps before the control and still names the
    // title key.
    function test_footer_hints_fit_beside_delete() {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(historyActionsC, root);
        const meetingsActions = createTemporaryObject(meetingsActionsC, root);
        const widths = [Theme.appWindowWidth - Theme.sidebarWidth, Theme.appWindowMinWidth - Theme.sidebarWidth];
        for (const width of widths) {
            const pane = createTemporaryObject(detailPaneC, root, {
                "width": width,
                "height": 800,
                "meetingId": "m1",
                "detail": detail,
                "actions": actions,
                "meetingsActions": meetingsActions
            });
            waitForRendering(pane);
            const hints = findByName(pane, "Keyboard hints");
            const remove = findByName(pane, "Delete meeting");
            verify(hints && remove);
            const hintsRight = hints.mapToItem(pane, hints.width, 0).x;
            const removeLeft = remove.mapToItem(pane, 0, 0).x;
            verify(hintsRight <= removeLeft, "hints end at " + hintsRight + ", delete starts at " + removeLeft + " at width " + width);
            compare(hints.truncated, false);
            verify(hints.text.indexOf("t title") >= 0);
            compare(hints.lineCount, width === widths[0] ? 1 : 3);
            pane.destroy();
        }
    }

    Component {
        id: stripC
        ProcessingStrip {}
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
        id: detailPaneC
        MeetingDetail {}
    }
}
