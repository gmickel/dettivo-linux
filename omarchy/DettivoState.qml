pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "DettivoText.js" as DettivoText

// Everything the widget, the panel and the pill know about Dettivo, read
// from the `dettivo` command and nothing else: one long-lived
// `dettivo --json events --follow` for the life of the shell, and the
// short `dettivo --json` calls of DettivoFacts for each fact and each
// action. The daemon may come and go; the stream restarts with the
// pill's backoff (half a second to eight) and every fact is re-read when
// it is back. No audio, no models and no socket are touched here (FR-B7).
Singleton {
    id: root

    // The daemon version the plugin needs; equals `omarchy.minDettivo` in
    // manifest.json (the repository's lint keeps the two in step).
    readonly property string minDettivo: "0.1.0"
    readonly property string binary: Quickshell.env("DETTIVO_CLI") || "dettivo"
    readonly property string moduleDir: Quickshell.env("DETTIVO_QML_DIR") || "/usr/lib/dettivo/qml/Dettivo"

    // The install facts.
    property bool moduleAvailable: false
    property bool binaryAvailable: true
    property string daemonVersion: ""
    property bool daemonAvailable: false
    readonly property bool versionOk: root.daemonVersion === "" || DettivoText.compareVersions(root.daemonVersion, root.minDettivo) >= 0
    readonly property string hint: (!root.binaryAvailable || !root.moduleAvailable) ? "install" : (root.versionOk ? "" : "upgrade")
    readonly property string hintDetail: root.hint === "upgrade" ? "daemon " + root.daemonVersion + ", plugin needs " + root.minDettivo : ""

    // The live state from the event stream, restored from the snapshots
    // (`dictation status`, `meetings list`) whenever the stream (re)opens.
    property string dictationState: "idle"
    property string engineLabel: ""
    property real level: 0
    property bool meetingActive: false
    property bool meetingRecording: false
    readonly property bool meetingFinalising: root.meetingActive && !root.meetingRecording
    property string meetingId: ""
    property string meetingElapsed: ""
    property string insertedTarget: ""
    // Counts every session event; a snapshot that started before a newer
    // event arrived is stale and never overwrites what the event said.
    property int sessionSeq: 0

    // The pill (Dettivo.Osd): its state and texts.
    property string pillState: "hidden"
    property string pillTitle: ""
    property string pillHint: ""
    property string pillWords: ""
    property string pillTarget: ""
    property string pillReason: ""
    property string pillAction: ""
    readonly property bool pillShown: root.pillState !== "hidden"

    // The facts the panel shows (DettivoFacts fills them).
    property int modeIndex: 0
    property string engineFact: ""
    property string enhancedFact: ""
    property string insertFact: ""
    property var recent: []

    // [omarchy] and [osd] from config.toml.
    property string glyph: "waveform"
    property bool levelMeter: true
    property string osdMode: "panel"
    property int historyItems: 3
    property string openShortcut: ""
    property string osdPosition: "top"
    property int osdMargin: 24

    readonly property string panelState: {
        if (root.hint !== "")
            return "hint";
        if (!root.daemonAvailable)
            return "unavailable";
        if (root.dictationState === "recording")
            return "recording";
        if (root.dictationState === "transcribing" || root.dictationState === "inserting" || root.meetingFinalising)
            return "transcribing";
        if (root.dictationState === "inserted")
            return "inserted";
        if (root.meetingActive)
            return "meeting";
        return "idle";
    }
    readonly property string glyphState: {
        if (root.dictationState === "recording")
            return "listening";
        if (root.dictationState === "transcribing" || root.dictationState === "inserting" || root.meetingFinalising)
            return "transcribing";
        if (root.dictationState === "failed")
            return "error";
        if (root.meetingActive)
            return "meeting";
        return "idle";
    }
    readonly property bool dimmed: root.hint !== "" || !root.daemonAvailable

    // Runs one action through the command; nothing here waits for it.
    function run(args) {
        Quickshell.execDetached([root.binary, "--quiet"].concat(args));
    }

    function dictate() {
        root.run(["dictation", "start"]);
    }
    function stopDictation() {
        root.run(["dictation", "stop"]);
    }
    function cancelDictation() {
        root.run(["dictation", "cancel"]);
    }
    function startMeeting() {
        root.meetingActions.start();
    }
    function stopMeeting() {
        root.meetingActions.stop();
    }
    function selectMode(index) {
        const modes = ["raw", "polish", "enhanced"];
        root.modeIndex = index;
        root.run(["config", "set", "dictation.mode", modes[index] || "raw"]);
    }
    function openApp(route) {
        root.run(["app", "open", route]);
    }
    function setSetting(key, value) {
        root.run(["config", "set", "omarchy." + key, String(value)]);
    }

    function showPill(state, texts) {
        root.pillTitle = texts.title || "";
        root.pillHint = texts.hint || "";
        root.pillWords = texts.words || "";
        root.pillTarget = texts.target || "";
        root.pillReason = texts.reason || "";
        root.pillAction = texts.action || "";
        root.pillState = state;
    }

    function pillHidden() {
        root.pillState = "hidden";
        if (root.dictationState === "inserted" || root.dictationState === "failed")
            root.dictationState = "idle";
    }

    function handleEvent(line) {
        let event;
        try {
            event = JSON.parse(line);
        } catch (e) {
            return;
        }
        const payload = event.payload || {};
        switch (event.topic) {
        case "dictation.state":
            root.sessionSeq += 1;
            root.handleDictation(payload);
            break;
        case "audio.level":
            root.level = Number(payload.rms !== undefined ? payload.rms : payload.peak) || 0;
            break;
        case "engine.state":
            root.engineLabel = [payload.model, payload.backend].filter(p => p).join(" · ");
            break;
        case "meeting.state":
            root.sessionSeq += 1;
            root.setMeeting(payload.state, payload.meeting_id, payload.duration_ms !== undefined ? payload.duration_ms : payload.live_last_end_ms);
            break;
        case "events.overflow":
            root.snapshots.refresh();
            break;
        }
    }

    function handleDictation(payload) {
        const state = String(payload.state || "");
        if (state === "recording") {
            root.dictationState = "recording";
            root.showPill("listening", {
                "hint": "release to insert"
            });
        } else if (state === "transcribing") {
            root.dictationState = "transcribing";
            root.showPill("transcribing", {});
        } else if (state === "inserting") {
            root.dictationState = "inserting";
        } else if (state === "idle" && payload.previous_state === "inserting") {
            // The completion: the daemon attaches the insertion to the
            // idle transition (docs/api/linux-deltas.md).
            root.handleCompletion(payload.insertion || {}, payload.first_words || "");
            root.facts.refreshHistory();
        } else if (state === "failed") {
            root.dictationState = "failed";
            root.showPill("error", {
                "title": "Dictation failed",
                "reason": payload.reason || ""
            });
        } else if (state === "idle" && payload.previous_state === "failed") {
            // The idle that follows a failure at once; the pill keeps the reason.
        } else if (state === "cancelled" || state === "idle") {
            root.dictationState = "idle";
            root.pillState = "hidden";
        }
    }

    function handleCompletion(insertion, words) {
        if (insertion.outcome === "inserted") {
            const app = insertion.target_app || {};
            root.insertedTarget = app.name || app.bundle_id || "";
            root.dictationState = "inserted";
            root.showPill("inserted", {
                "target": root.insertedTarget,
                "words": words
            });
        } else if (insertion.outcome === "copied_to_clipboard") {
            root.dictationState = "idle";
            root.showPill("copied", {
                "reason": insertion.reason || "",
                "action": "Ctrl+V",
                "words": words
            });
        } else {
            root.dictationState = "failed";
            root.showPill("error", {
                "title": "Not inserted",
                "reason": insertion.reason || "the insertion failed",
                "action": "copy from history"
            });
        }
    }

    // elapsedMs: what the meeting has recorded so far, when known.
    function setMeeting(state, id, elapsedMs) {
        root.meetingClock.apply(state, id, elapsedMs);
    }

    function refresh() {
        root.facts.refresh();
        root.snapshots.refresh();
    }

    Component.onCompleted: {
        root.moduleCheck.running = true;
        root.refresh();
    }

    readonly property DettivoFacts facts: DettivoFacts {
        state: root
    }

    readonly property Process moduleCheck: Process {
        command: ["test", "-f", root.moduleDir + "/qmldir"]
        onExited: code => {
            root.moduleAvailable = code === 0;
        }
    }

    readonly property Process events: Process {
        command: [root.binary, "--json", "events", "--follow"]
        running: true
        stdout: SplitParser {
            onRead: data => root.handleEvent(data)
        }
        onStarted: root.retry.interval = 500
        onExited: code => {
            root.sessionSeq += 1;
            root.daemonAvailable = false;
            root.binaryAvailable = code !== 127;
            root.pillState = "hidden";
            root.dictationState = "idle";
            root.setMeeting("completed", "", undefined);
            root.retry.start();
        }
    }

    readonly property Timer retry: Timer {
        interval: 500
        repeat: false
        onTriggered: {
            root.retry.interval = Math.min(root.retry.interval * 2, 8000);
            root.events.running = true;
            root.refresh();
        }
    }

    readonly property DettivoSnapshots snapshots: DettivoSnapshots {
        state: root
    }

    readonly property DettivoMeetingActions meetingActions: DettivoMeetingActions {
        state: root
    }

    readonly property DettivoMeetingClock meetingClock: DettivoMeetingClock {
        state: root
    }
}
