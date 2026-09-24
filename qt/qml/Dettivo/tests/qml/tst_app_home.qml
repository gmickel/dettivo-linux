import Dettivo
import QtQuick
import QtTest

// Home's pieces (fn-17): the instrument strip binds the mode and the
// actions to the daemon's facts, Today lists the newest day or shows the
// designed empty state, and the rail draws the engine states with the
// progress hairline.
TestCase {
    id: root

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

    function test_instrument_strip_binds_mode_and_actions() {
        const status = createTemporaryObject(statusC, root);
        const config = createTemporaryObject(configC, root);
        const strip = createTemporaryObject(stripC, root, {
            "status": status,
            "config": config,
            "width": 1000
        });
        waitForRendering(strip);
        compare(strip.mode, "polish");
        const segment = findByName(strip, "Dictation mode");
        verify(segment);
        compare(segment.currentIndex, 1);
        const start = findByName(strip, "Start dictation");
        verify(start);
        compare(start.enabled, true);
        mouseClick(start);
        compare(status.toggles, 1);
        status.dictationState = "recording";
        verify(findByName(strip, "Stop dictation"));
        compare(strip.recording, true);
        const wave = findByName(strip, "Audio level");
        verify(wave);
        compare(wave.barColor, Theme.roleAccent);
        status.daemonConnected = false;
        compare(findByName(strip, "Stop dictation").enabled, false);
        mouseClick(segment.segmentAt(0));
        compare(config.writes[0][0], "dictation.mode");
        compare(config.writes[0][1], "raw");
    }

    function test_start_meeting_is_available_when_the_daemon_is_idle() {
        const status = createTemporaryObject(statusC, root);
        const strip = createTemporaryObject(stripC, root, {
            "status": status,
            "width": 1000
        });
        waitForRendering(strip);
        const start = findByName(strip, "Start meeting");
        verify(start);
        compare(start.enabled, true);
        status.dictationState = "recording";
        compare(start.enabled, false);
        status.dictationState = "idle";
        status.daemonConnected = false;
        compare(start.enabled, false);
    }

    function test_today_empty_state_names_the_configured_hold_chord() {
        const status = createTemporaryObject(statusC, root, {
            "holdChord": "F8"
        });
        const list = createTemporaryObject(todayListC, root, {
            "status": status,
            "width": 700,
            "height": 400
        });
        waitForRendering(list);
        compare(list.count, 0);
        verify(findByName(list, "Hold F8 in any app and it will show up here."));
        verify(findByName(list, "F8"));
        status.holdChord = "";
        verify(findByName(list, "Hold F9 in any app and it will show up here."));
        const bare = createTemporaryObject(todayListC, root, {
            "width": 700,
            "height": 400
        });
        waitForRendering(bare);
        verify(findByName(bare, "Hold F9 in any app and it will show up here."));
    }

    function test_today_lists_rows_or_the_empty_state() {
        const model = createTemporaryObject(todayC, root);
        const router = createTemporaryObject(routerC, root);
        const list = createTemporaryObject(todayListC, root, {
            "model": model,
            "router": router,
            "width": 700,
            "height": 400
        });
        waitForRendering(list);
        compare(list.count, 2);
        const row = findByName(list, "Add a regression test");
        verify(row);
        compare(row.horizontalPadding, 0);
        compare(row.leading, "13:12");
        compare(row.trailing, "dictation · 4 s");
        const meeting = findByName(list, "Dettivo Linux kickoff");
        verify(meeting);
        compare(meeting.trailing, "41 min");
        mouseClick(row);
        compare(router.page, "history.detail");
        compare(router.arg, "");
        list.move(1);
        verify(meeting.activeFocus);
        keyClick(Qt.Key_Return);
        compare(router.page, "meetings.detail");
        list.move(-1);
        verify(row.activeFocus);
        compare(findByName(list, "Nothing dictated yet.").visible, false);
        model.clear();
        tryCompare(list, "count", 0);
        compare(findByName(list, "Nothing dictated yet.").visible, true);
    }

    function test_rail_draws_engine_states_and_progress() {
        const engines = createTemporaryObject(enginesC, root);
        const status = createTemporaryObject(statusC, root);
        const rail = createTemporaryObject(railC, root, {
            "engines": engines,
            "status": status,
            "width": 320,
            "height": 500
        });
        waitForRendering(rail);
        const warm = findByName(rail, "parakeet v3: warm");
        verify(warm);
        compare(warm.accent, true);
        compare(warm.busy, false);
        const track = findChild(warm, "engineProgressTrack");
        verify(track);
        verify(track.y + track.height < warm.height);
        compare(track.width, warm.width);
        const loading = findByName(rail, "whisper small: downloading");
        verify(loading);
        compare(loading.busy, true);
        fuzzyCompare(loading.progress, 0.5, 0.01);
        verify(findByName(rail, "Engines"));
        verify(findByName(rail, "Agents"));
        verify(findByName(rail, "socket: peer"));
        verify(findByName(rail, "mcp: not checked"));
    }

    Component {
        id: routerC

        QtObject {
            property string route: "home"
            property string sub: ""
            property string arg: ""
            property bool detail: false
            property var settingsSections: ["general", "hotkeys", "models", "polish", "insertion", "meetings", "agents", "diagnostics"]
            property var opened: []
            readonly property string page: sub.length > 0 ? route + "." + sub : route
            readonly property string title: route

            function open(name) {
                opened.push(name);
                const dot = name.indexOf(".");
                route = dot > 0 ? name.slice(0, dot) : (name === "agents" ? "settings" : name);
                sub = dot > 0 ? name.slice(dot + 1) : (name === "agents" ? "agents" : "");
                return true;
            }

            function back() {
                sub = "";
            }
        }
    }

    Component {
        id: statusC

        QtObject {
            property bool daemonConnected: true
            property string daemonState: "connected"
            property bool healthOk: true
            property string dictationState: "idle"
            property string holdChord: "F9"
            property string toggleChord: "Super+Ctrl+X"
            property string targetApp: "ghostty"
            property string socketMode: "peer"
            property string gpu: "vulkan"
            property string inputName: "Arctis Nova"
            property string inputRate: "16 kHz"
            property string restState: "off"
            property string mcpHosts: ""
            property string lastCall: ""
            property string clock: "Wed 3 Sep · 13:42"
            property var levels: [0.1, 0.5, 0.9]
            property int toggles: 0

            function toggleDictation() {
                toggles += 1;
            }
        }
    }

    Component {
        id: enginesC
        FakeHomeEngines {}
    }

    Component {
        id: todayC

        ListModel {
            ListElement {
                itemId: "a"

                kind: "dictation"
                title: "Add a regression test"
                time: "13:12"
                duration: "4 s"
            }

            ListElement {
                itemId: "b"

                kind: "meeting"
                title: "Dettivo Linux kickoff"
                time: "11:05"
                duration: "41 min"
            }
        }
    }

    Component {
        id: configC

        QtObject {
            property int revision: 1
            property var writes: []

            function value(key) {
                return key === "dictation.mode" ? "polish" : undefined;
            }

            function set(key, v) {
                writes.push([key, v]);
            }
        }
    }

    Component {
        id: stripC

        InstrumentStrip {}
    }

    Component {
        id: todayListC

        TodayList {}
    }

    Component {
        id: railC

        RightRail {}
    }

    name: "AppHome"
    width: 1280
    height: 820
    visible: true
    when: windowShown
}
