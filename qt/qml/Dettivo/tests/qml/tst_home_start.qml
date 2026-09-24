import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "HomeStart"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    Component {
        id: windowC
        AppWindow {}
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
        id: enginesC
        FakeHomeEngines {}
    }
    Component {
        id: routerC
        QtObject {
            property string route: "home"
            property string sub: ""
            property string arg: ""
            readonly property bool detail: sub.length > 0
            readonly property string page: sub ? route + "." + sub : route
            readonly property string title: route
            function open(name) {
                const parts = name.split(".");
                route = parts[0];
                sub = parts.length > 1 ? parts[1] : "";
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
            property string inputName: "Microphone"
            property string inputRate: "16 kHz"
            property string restState: "off"
            property string mcpHosts: ""
            property string lastCall: ""
            property string clock: "12:00"
            property var levels: []
        }
    }

    function named(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const nested = named(child, name);
            if (nested)
                return nested;
        }
        return null;
    }

    Component {
        id: railC
        RightRail {
            width: 320
            height: 800
        }
    }

    function test_unobserved_hosts_are_not_reported_as_absent() {
        const status = createTemporaryObject(statusC, root);
        const rail = createTemporaryObject(railC, root, {
            status: status
        });
        verify(named(rail, "mcp: not checked") !== null);
        status.mcpHosts = "none configured";
        verify(named(rail, "mcp: none configured") !== null);
        status.mcpHosts = "codex";
        verify(named(rail, "mcp: codex") !== null);
    }

    function test_home_start_uses_existing_meeting_flow_data() {
        return [
            {
                tag: "idle",
                active: false
            },
            {
                tag: "existing-meeting",
                active: true
            }
        ];
    }

    function test_home_start_uses_existing_meeting_flow(data) {
        const router = createTemporaryObject(routerC, root);
        const live = createTemporaryObject(liveC, root, {
            active: data.active
        });
        const window = createTemporaryObject(windowC, root, {
            router: router,
            status: createTemporaryObject(statusC, root),
            engines: createTemporaryObject(enginesC, root),
            meetingLive: live,
            meetingsActions: createTemporaryObject(actionsC, root),
            visible: true
        });
        const start = named(window.contentItem, data.active ? "Return to meeting" : "Start meeting");
        verify(start);
        verify(start.enabled);
        mouseClick(start);
        tryCompare(router, "page", data.active ? "meetings.live" : "meetings");
        if (data.active) {
            compare(live.starts.length, 0);
        } else {
            tryVerify(() => live.starts.length === 1);
            compare(live.starts[0].provider, "whisper");
            compare(live.starts[0].analyze, undefined);
            compare(live.starts[0].diarize, undefined);
        }
        window.close();
    }
}
