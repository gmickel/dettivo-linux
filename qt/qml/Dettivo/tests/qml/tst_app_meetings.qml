import Dettivo
import QtQuick
import QtTest

// The meetings list side (fn-34, ADR 0038): a row carries its chip, its
// swatches and its partial actions, the chip takes its tone from the
// state, the swatches read the speaker roles, the source row toggles,
// and the route keeps the list beside the rail with the route's names
// and opens rows; tst_app_meetings_rail.qml covers the new-meeting rail
// and tst_app_meetings_detail.qml the live and detail screens. The fakes
// are the Fake*.qml files beside this one.
TestCase {
    id: root

    name: "AppMeetings"
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

    // True when the item or one of its ancestors is hidden (a closed
    // popup keeps its content's own `visible` flag).
    function hiddenUp(item) {
        if (item === null)
            return true;
        for (let at = item; at; at = at.parent) {
            if (!at.visible)
                return true;
        }
        return false;
    }

    function test_row_carries_chip_swatches_and_partial_actions() {
        const actions = createTemporaryObject(actionsC, root);
        const row = createTemporaryObject(rowC, root, {
            "width": 760,
            "meetingId": "m3",
            "when": "Mon 16:00",
            "date": "1 Sep",
            "title": "Customer call",
            "summary": "",
            "length": "52 min",
            "speakers": [
                {
                    "name": "You",
                    "colorIndex": 0,
                    "speakerId": "you"
                },
                {
                    "name": "Remote",
                    "colorIndex": 1,
                    "speakerId": "remote"
                }
            ],
            "speakerCount": 2,
            "chip": "Partial",
            "chipKind": "partial",
            "partial": true,
            "recoverable": true,
            "actions": actions
        });
        waitForRendering(row);
        compare(row.Accessible.name, "Customer call");
        compare(row.summaryLine, "Recovered after a crash");
        verify(findByName(row, "Partial"));
        verify(findByName(row, "You, Remote"));
        mouseClick(findByName(row, "Discard"));
        compare(actions.discarded[0], "m3");
        mouseClick(findByName(row, "Recover"));
        compare(actions.recovered[0], "m3");
        const activated = [];
        row.activated.connect(() => activated.push(true));
        mouseClick(row, 400, 20);
        compare(activated.length, 1);
    }

    function test_chip_and_swatches_take_their_tone_from_the_state() {
        const analysed = createTemporaryObject(chipC, root, {
            "text": "Analysed",
            "kind": "analysed"
        });
        const partial = createTemporaryObject(chipC, root, {
            "text": "Partial",
            "kind": "partial"
        });
        const notes = createTemporaryObject(chipC, root, {
            "text": "Notes only",
            "kind": "notes"
        });
        compare(analysed.tone, Theme.roleAccent);
        compare(partial.tone, Theme.roleUrgent);
        compare(notes.tone, Theme.roleMutedText);
        const swatches = createTemporaryObject(swatchesC, root, {
            "width": 168,
            "count": 3,
            "speakers": []
        });
        compare(swatches.names, "3 speakers");
        swatches.speakers = [
            {
                "name": "You",
                "colorIndex": 0,
                "speakerId": "you"
            },
            {
                "name": "Mara",
                "colorIndex": 1,
                "speakerId": "s0"
            }
        ];
        compare(swatches.names, "You, Mara");
        compare(swatches.colors.length, Theme.roleSpeakerColors.length);
    }

    function test_source_row_toggles_unless_fixed() {
        const source = createTemporaryObject(sourceC, root, {
            "width": 280,
            "name": "System audio",
            "device": "monitor of the default sink",
            "checked": true
        });
        waitForRendering(source);
        const toggled = [];
        source.toggled.connect(() => toggled.push(true));
        mouseClick(findByName(source, "System audio"));
        compare(toggled.length, 1);
        const mic = createTemporaryObject(sourceC, root, {
            "width": 280,
            "name": "Microphone",
            "fixed": true
        });
        waitForRendering(mic);
        mouseClick(findByName(mic, "Microphone"));
        compare(mic.checked, true);
        verify(findByName(mic, "Microphone level"));
    }

    function test_route_keeps_list_beside_rail_and_opens_rows_data() {
        return [
            {
                tag: "normal",
                pageWidth: 1080
            },
            {
                tag: "minimum",
                pageWidth: Theme.appWindowMinWidth - Theme.sidebarWidth
            }
        ];
    }

    function test_route_keeps_list_beside_rail_and_opens_rows(data) {
        const router = createTemporaryObject(routerC, root);
        const meetings = createTemporaryObject(meetingsC, root);
        const live = createTemporaryObject(liveC, root);
        const actions = createTemporaryObject(actionsC, root);
        const page = createTemporaryObject(routeC, root, {
            "width": data.pageWidth,
            "height": 800,
            "router": router,
            "meetings": meetings,
            "live": live,
            "meetingsActions": actions
        });
        waitForRendering(page);
        compare(page.Accessible.name, "Meetings");
        verify(findByName(page, "Meetings list"));
        verify(findByName(page, "New meeting rail"));
        verify(findByName(page, "Search meetings"));
        verify(findByName(page, "This week"));
        meetings.append(meetings.get(0));
        const viewport = findByName(page, "Meetings table");
        viewport.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_J);
        compare(page.pageItem.currentIndex, 0);
        keyClick(Qt.Key_J);
        compare(page.pageItem.currentIndex, 1);
        keyClick(Qt.Key_K);
        compare(page.pageItem.currentIndex, 0);
        keyClick(Qt.Key_Return);
        compare(router.opened[router.opened.length - 1], "meetings.detail:m1");
        const row = findByName(page, "Dettivo Linux kickoff");
        verify(row);
        mouseClick(row);
        compare(router.opened[router.opened.length - 1], "meetings.detail:m1");
        compare(page.focusSearch(), true);
        const queries = [];
        findByName(page, "Meetings search").queryChanged.connect(query => queries.push(query));
        keyClick(Qt.Key_J);
        keyClick(Qt.Key_Return);
        compare(queries[queries.length - 1], "j");
        compare(page.pageItem.currentIndex, 0);
        compare(page.dismiss(), true);
        compare(page.dismiss(), false);
        page.importAudio();
        const overlay = root.Window.window.contentItem;
        tryVerify(() => {
            return findByName(overlay, "Import dialog") !== null;
        });
        compare(page.dismiss(), true);
        live.disclosureRequired("May be recorded.");
        tryVerify(() => {
            return findByName(overlay, "Recording disclosure") !== null;
        });
        mouseClick(findByName(overlay, "Not now"));
        compare(live.dismissed, 1);
        router.sub = "live";
        compare(page.Accessible.name, "Meeting live");
        router.sub = "detail";
        router.arg = "m1";
        compare(page.Accessible.name, "Meeting");
        // `t` opens the title for renaming, and the saved title reaches
        // the list without a reload.
        page.detail = createTemporaryObject(detailC, root);
        tryVerify(() => findByName(page, "Rename meeting") !== null);
        // The disclosure dialog is still leaving (its exit motion); a modal
        // popup holds the keys until it is gone.
        tryVerify(() => root.hiddenUp(findByName(overlay, "Recording disclosure")));
        page.forceActiveFocus();
        keyClick(Qt.Key_T);
        tryVerify(() => findByName(page, "Meeting title") !== null && findByName(page, "Meeting title").visible);
        keyClick(Qt.Key_Escape);
        actions.retitled("m1", "Kickoff, renamed");
        compare(meetings.retitles[0], "m1:Kickoff, renamed");
        compare(meetings.get(0).title, "Kickoff, renamed");
    }

    Component {
        id: detailC
        FakeMeetingDetail {}
    }

    Component {
        id: routerC
        FakeMeetingsRouter {}
    }

    Component {
        id: meetingsC
        FakeMeetingsModel {}
    }

    Component {
        id: liveC
        FakeMeetingLive {}
    }

    Component {
        id: actionsC
        FakeMeetingsActions {}
    }

    Component {
        id: rowC
        MeetingRow {}
    }

    Component {
        id: chipC
        StateChip {}
    }

    Component {
        id: swatchesC
        SpeakerSwatches {}
    }

    Component {
        id: sourceC
        SourceRow {}
    }

    Component {
        id: routeC
        MeetingsRoute {}
    }
}
