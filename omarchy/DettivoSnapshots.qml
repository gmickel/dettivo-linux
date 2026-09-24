import QtQuick
import Quickshell.Io

// The two session snapshots DettivoState re-reads whenever the event
// stream (re)opens or overflows: `dictation status` for the dictation
// in flight and `meetings list --limit 1` for a meeting still recording.
// Each probe remembers the event count it started at and drops its
// answer when an event arrived meanwhile, so a late snapshot never
// overwrites what the stream said. Owned by DettivoState, which is
// `state` here.
QtObject {
    id: root

    required property var state
    property int probeStartedAt: 0
    property int meetingProbeStartedAt: 0

    function refresh() {
        if (!probe.running) {
            root.probeStartedAt = root.state.sessionSeq;
            probe.running = true;
        }
        if (!meetingProbe.running) {
            root.meetingProbeStartedAt = root.state.sessionSeq;
            meetingProbe.running = true;
        }
    }

    // A session in flight is recording until its job reports the
    // transcription; a finished one is idle unless the pill still shows
    // the insertion.
    function applyDictationSnapshot(status) {
        if (status.is_active) {
            const job = status.job || {};
            const message = String(job.message || "");
            root.state.dictationState = message.indexOf("listening") === 0 ? "recording" : "transcribing";
            root.state.pillState = "hidden";
        } else if (root.state.dictationState !== "inserted") {
            root.state.dictationState = "idle";
        }
    }

    // The newest meeting is the one that can still be recording; its
    // started_at anchors the timer again.
    function applyMeetingSnapshot(list) {
        const items = list.items || [];
        const newest = items.length > 0 ? items[0] : null;
        if (newest && (newest.status === "recording" || newest.status === "transcribing"))
            root.state.setMeeting(newest.status, newest.ref ? newest.ref.id : "", root.recordedSoFar(newest.started_at));
        else
            root.state.setMeeting("completed", "", undefined);
    }

    // Milliseconds since the stamp, or undefined when it is missing or unreadable.
    function recordedSoFar(startedAt) {
        const started = Date.parse(String(startedAt || ""));
        return isNaN(started) ? undefined : Math.max(0, root.state.meetingClock.now() - started);
    }

    readonly property Process probe: Process {
        command: [root.state.binary, "--json", "dictation", "status"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const status = JSON.parse(text);
                    root.state.daemonAvailable = true;
                    if (root.probeStartedAt === root.state.sessionSeq)
                        root.applyDictationSnapshot(status);
                } catch (e) {}
            }
        }
        onExited: code => {
            if (code !== 0)
                root.state.daemonAvailable = false;
            if (root.probeStartedAt !== root.state.sessionSeq)
                root.refresh();
        }
    }

    readonly property Process meetingProbe: Process {
        command: [root.state.binary, "--json", "meetings", "list", "--limit", "1"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const list = JSON.parse(text);
                    if (root.meetingProbeStartedAt === root.state.sessionSeq)
                        root.applyMeetingSnapshot(list);
                } catch (e) {}
            }
        }
        onExited: {
            if (root.meetingProbeStartedAt !== root.state.sessionSeq)
                root.refresh();
        }
    }
}
