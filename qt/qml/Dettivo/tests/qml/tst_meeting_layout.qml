import Dettivo
import QtQuick
import QtQuick.Controls
import QtTest

TestCase {
    id: root
    name: "MeetingLayout"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function named(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function test_transcript_fits_after_resize_and_follows_growing_tail() {
        const model = createTemporaryObject(modelC, root);
        for (let i = 0; i < 4; i++)
            model.append({
                time: "11:05",
                label: "You",
                colorIndex: 0,
                gapBefore: 0,
                provisional: false,
                text: "A transcript line with enough words to wrap across the recording window."
            });
        const live = createTemporaryObject(liveC, root, {
            segments: model
        });
        const page = createTemporaryObject(transcriptC, root, {
            width: 700,
            height: 500,
            live: live
        });
        waitForRendering(page);
        const list = named(page, "Live transcript");
        tryVerify(() => list.contentHeight < list.height);
        tryCompare(list, "contentY", list.originY);
        page.height = 130;
        waitForRendering(page);
        model.append({
            time: "11:06",
            label: "You",
            colorIndex: 0,
            gapBefore: 0,
            provisional: true,
            text: "A growing provisional tail. ".repeat(12)
        });
        tryVerify(() => list.atYEnd);
        model.setProperty(4, "text", "A growing provisional tail. ".repeat(24));
        waitForRendering(page);
        tryVerify(() => list.atYEnd);
        mousePress(list, 300, 30);
        mouseMove(list, 300, 80, 30);
        mouseMove(list, 300, 90, 30);
        tryVerify(() => list.dragging);
        const readingAt = list.contentY;
        model.setProperty(4, "text", "A growing provisional tail. ".repeat(36));
        waitForRendering(page);
        compare(list.contentY, readingAt);
        mouseRelease(list, 300, 90);
        list.cancelFlick();
        list.positionViewAtEnd();
        list.movementEnded();
        waitForRendering(page);
        const bar = list.ScrollBar.vertical;
        mousePress(bar, bar.width / 2, bar.height - 3);
        verify(bar.pressed);
        waitForRendering(page);
        const heldAt = list.contentY;
        const heightBefore = list.contentHeight;
        model.setProperty(4, "text", "A growing provisional tail. ".repeat(48));
        waitForRendering(page);
        tryVerify(() => list.contentHeight > heightBefore);
        wait(50);
        compare(Math.round(list.contentY), Math.round(heldAt));
        mouseRelease(bar, bar.width / 2, bar.height - 3);
        page.height = 800;
        page.width = 1000;
        tryVerify(() => list.contentHeight < list.height);
        tryCompare(list, "contentY", list.originY);
    }

    function test_analysis_scrolls_long_content_clear_of_facts() {
        const detail = createTemporaryObject(detailC, root, {
            summary: "A long meeting summary. ".repeat(80)
        });
        const rail = createTemporaryObject(railC, root, {
            width: 320,
            height: 360,
            detail: detail
        });
        waitForRendering(rail);
        const summary = named(rail, detail.summary);
        verify(summary);
        let viewport = summary.parent;
        while (viewport && viewport.contentHeight === undefined)
            viewport = viewport.parent;
        verify(viewport !== null, "Long analysis needs a scroll viewport");
        verify(viewport.clip);
        verify(viewport.contentHeight > viewport.height);
        viewport.contentY = viewport.contentHeight - viewport.height;
        waitForRendering(rail);
        const facts = named(rail, "audio: " + detail.audioFacts);
        verify(facts);
        verify(facts.mapToItem(rail, 0, 0).y >= viewport.y + viewport.height);
    }

    function test_long_engine_value_is_readable() {
        const value = "whisper.cpp · whisper-large-v3-turbo · Vulkan";
        const row = createTemporaryObject(factC, root, {
            width: 280,
            name: "engine",
            value: value
        });
        waitForRendering(row);
        const label = row.children.find(child => child.text === value);
        verify(label);
        compare(label.truncated, false);
        verify(label.contentHeight <= row.height);
    }

    function test_analysis_action_focus_reveals_long_failure() {
        const detail = createTemporaryObject(detailC, root, {
            analysisStatus: "failed",
            analysisError: "The model could not finish this meeting. ".repeat(40)
        });
        const actions = createTemporaryObject(actionsC, root);
        const rail = createTemporaryObject(railC, root, {
            width: 320,
            height: 360,
            detail: detail,
            actions: actions,
            meetingId: "m1"
        });
        waitForRendering(rail);
        const button = named(rail, "Analyse now");
        verify(button);
        button.forceActiveFocus(Qt.TabFocusReason);
        const facts = named(rail, "notes: " + detail.notesMeta);
        tryVerify(() => button.mapToItem(rail, 0, 0).y >= 0 && button.mapToItem(rail, 0, button.height).y < facts.mapToItem(rail, 0, 0).y);
        mouseClick(button);
        compare(actions.analysed[0], "m1");
    }

    Component {
        id: actionsC
        QtObject {
            property bool busy: false
            property var analysed: []
            function analyze(id, force) {
                analysed.push(id);
            }
        }
    }
    Component {
        id: modelC
        ListModel {}
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
        id: transcriptC
        LiveTranscript {}
    }
    Component {
        id: railC
        AnalysisRail {}
    }
    Component {
        id: factC
        RailFactRow {}
    }
}
