import Dettivo
import QtQuick
import QtTest

// The app's shell (fn-17): the sidebar names every route and marks the
// open one with its three footer rows, the daemon banner shows and
// clears, a designed state carries its sentence, reason and key, and the
// window swaps pages by route with the route's accessible title.
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

    function test_sidebar_names_routes_and_marks_the_open_one() {
        const router = createTemporaryObject(routerC, root);
        const engines = createTemporaryObject(enginesC, root);
        const status = createTemporaryObject(statusC, root);
        const sidebar = createTemporaryObject(sidebarC, root, {
            "router": router,
            "engines": engines,
            "status": status,
            "height": 820
        });
        waitForRendering(sidebar);
        compare(sidebar.Accessible.name, "Sidebar");
        const home = findByName(sidebar, "Home");
        verify(home);
        compare(home.Accessible.selected, true);
        const history = findByName(sidebar, "History");
        verify(history);
        compare(history.Accessible.selected, false);
        mouseClick(history);
        compare(router.opened[router.opened.length - 1], "history");
        compare(history.active, true);
        compare(home.active, false);
        const agents = findByName(sidebar, "Agents");
        mouseClick(agents);
        compare(router.page, "settings.agents");
        compare(agents.active, true);
        verify(findByName(sidebar, "Speech engine: parakeet v3, warm"));
        verify(findByName(sidebar, "Language model: qwen3 4b, idle"));
        verify(findByName(sidebar, "Socket mode: socket, peer"));
    }

    function test_daemon_banner_shows_away_and_clears() {
        const status = createTemporaryObject(statusC, root);
        const banner = createTemporaryObject(bannerC, root, {
            "status": status,
            "width": 800
        });
        compare(banner.shown, false);
        compare(banner.visible, false);
        status.daemonState = "away";
        compare(banner.shown, true);
        tryVerify(() => {
            return banner.height > 0;
        });
        verify(findByName(banner, "Daemon unavailable"));
        status.daemonState = "connected";
        compare(banner.shown, false);
        status.healthOk = false;
        verify(findByName(banner, "Configuration needs attention"));
    }

    function test_empty_state_carries_sentence_reason_and_key() {
        const state = createTemporaryObject(emptyC, root, {
            "width": 400
        });
        compare(state.Accessible.name, "Nothing dictated yet.");
        verify(findByName(state, "Hold F9 in any app and it will show up here."));
        verify(findByName(state, "F9"));
        compare(state.urgent, false);
        verify(state.complete);
    }

    function test_window_swaps_pages_by_route_with_the_title() {
        const router = createTemporaryObject(routerC, root);
        const status = createTemporaryObject(statusC, root);
        const window = createTemporaryObject(windowC, root, {
            "router": router,
            "status": status
        });
        verify(window);
        tryVerify(() => {
            return findByName(window.contentItem, "Home") !== null;
        });
        router.open("meetings.live");
        tryVerify(() => {
            return findByName(window.contentItem, "Meeting live") !== null;
        });
        router.open("settings.hotkeys");
        tryVerify(() => {
            return findByName(window.contentItem, "Settings / Hotkeys") !== null;
        });
        router.open("history.detail");
        tryVerify(() => {
            return findByName(window.contentItem, "Dictation") !== null;
        });
        verify(findByName(window.contentItem, "Keyboard hints").visible);
        // First run is the whole window: no sidebar, no hints (ADR 0024).
        router.open("onboarding");
        tryVerify(() => {
            return findByName(window.contentItem, "First run") !== null;
        });
        verify(!findByName(window.contentItem, "Keyboard hints").visible);
        verify(!findByName(window.contentItem, "Sidebar").visible);
        router.open("home");
        tryVerify(() => {
            return findByName(window.contentItem, "Home") !== null;
        });
        verify(findByName(window.contentItem, "Sidebar").visible);
    }

    // The smallest window the shell permits still lays every route out:
    // History's fixed list leaves its detail a rail's width, and the
    // meeting detail keeps room beside its rail.
    function test_minimum_width_leaves_every_route_room() {
        const router = createTemporaryObject(routerC, root);
        const status = createTemporaryObject(statusC, root);
        const window = createTemporaryObject(windowC, root, {
            "router": router,
            "status": status
        });
        verify(window);
        verify(window.minimumWidth >= Theme.sidebarWidth + Theme.historyListWidth + Theme.rightRailWidth);
        // A shown window lays its content out; the size is the minimum.
        window.width = window.minimumWidth;
        window.visible = true;
        tryVerify(() => {
            return window.contentItem.width === window.minimumWidth;
        });
        router.open("history.detail");
        let detail = null;
        tryVerify(() => {
            detail = findByName(window.contentItem, "Dictation detail");
            return detail !== null && detail.width > 0;
        });
        verify(detail.width >= Theme.rightRailWidth, "history detail width " + detail.width);
        verify(detail.x + detail.width <= window.width + 1, "history detail overflows " + (detail.x + detail.width));
        router.open("meetings.detail");
        let meeting = null;
        tryVerify(() => {
            meeting = findByName(window.contentItem, "Meeting detail");
            return meeting !== null && meeting.width > 0;
        });
        verify(meeting.width >= Theme.rightRailWidth + Theme.space8 + Theme.pagePaddingX * 2, "meeting detail width " + meeting.width);
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

        ListModel {
            property string speechName: "parakeet v3"
            property string speechState: "warm"
            property string languageModelName: "qwen3 4b"
            property string languageModelState: "idle"

            ListElement {
                name: "parakeet v3"
                state: "warm"
                detail: "vulkan"
                progress: 1
                busy: false
            }

            ListElement {
                name: "whisper small"
                state: "downloading"
                detail: ""
                progress: 0.5
                busy: true
            }
        }
    }

    Component {
        id: sidebarC

        Sidebar {}
    }

    Component {
        id: bannerC

        DaemonBanner {}
    }

    Component {
        id: emptyC

        StateView {
            title: "Nothing dictated yet."
            reason: "Hold F9 in any app and it will show up here."
            keyHint: "F9"
        }
    }

    Component {
        id: windowC

        AppWindow {
            visible: false
        }
    }

    name: "AppShell"
    width: 1280
    height: 820
    visible: true
    when: windowShown
}
