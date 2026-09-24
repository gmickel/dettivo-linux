import QtQuick
import QtTest
import Quickshell
import "plugin_support.js" as Support

// The meeting timer under the shell shim (fn-62, ADR 0063): it counts
// from the moment the recording started on the plugin's own clock, so a
// quiet room and a daemon that sends no further state keep it moving; a
// reopened stream restores the anchor from the snapshot's `started_at`;
// leaving `recording` stops the tick and clears the display.
TestCase {
    id: root
    name: "OmarchyPluginClock"
    width: 480
    height: 520
    visible: true
    when: windowShown

    readonly property string meetingId: "0f8fad5b-d9cb-469f-a165-70867728950e"
    readonly property real t0: Date.parse("2026-09-16T10:00:00Z")

    Component {
        id: widgetC
        BarWidget {}
    }

    function feed(events, topic, payload) {
        events.feed(JSON.stringify({
            "topic": topic,
            "payload": payload
        }));
    }

    function init() {
        // The singleton is created on first use; touch it before its
        // processes are answered.
        compare(DettivoState.binary, "dettivo");
        Support.readyDaemon(Quickshell);
        Support.processFor(Quickshell, ["meetings", "list"]).finish(JSON.stringify({
            items: []
        }));
        DettivoState.retry.stop();
        DettivoState.meetingClock.pinnedNowMs = root.t0;
    }

    function cleanup() {
        DettivoState.setMeeting("completed", "", undefined);
        DettivoState.meetingClock.pinnedNowMs = -1;
    }

    function test_1_the_timer_counts_from_the_recording_anchor_and_ticks() {
        const widget = createTemporaryObject(widgetC, root);
        const glyph = Support.glyphLoader(widget);
        tryCompare(glyph, "status", Loader.Ready);
        const events = Support.processFor(Quickshell, ["events", "--follow"]);
        verify(events);

        // The recording transition carries the capture so far; a quiet
        // room sends nothing more, and the display still advances.
        feed(events, "meeting.state", {
            "state": "recording",
            "meeting_id": root.meetingId,
            "live_segment_count": 0,
            "live_last_end_ms": 0,
            "duration_ms": 0
        });
        compare(DettivoState.meetingActive, true);
        compare(DettivoState.meetingRecording, true);
        compare(DettivoState.meetingElapsed, "00:00:00");
        compare(glyph.item.elapsed, "00:00");
        DettivoState.meetingClock.pinnedNowMs = root.t0 + 61000;
        tryCompare(DettivoState, "meetingElapsed", "00:01:01");
        compare(glyph.item.elapsed, "01:01");
        compare(widget.tooltip, "Dettivo — meeting 00:01:01");

        // A later state event re-anchors on the capture, not on the end
        // of the last live segment, which lags the audio.
        feed(events, "meeting.state", {
            "state": "recording",
            "meeting_id": root.meetingId,
            "live_last_end_ms": 30000,
            "duration_ms": 90000
        });
        compare(DettivoState.meetingElapsed, "00:01:30");
        // The macOS shape without `duration_ms` falls back to the live end.
        feed(events, "meeting.state", {
            "state": "recording",
            "meeting_id": root.meetingId,
            "live_last_end_ms": 1421000
        });
        compare(DettivoState.meetingElapsed, "00:23:41");
        DettivoState.meetingClock.pinnedNowMs = root.t0 + 61000 + 5000;
        tryCompare(DettivoState, "meetingElapsed", "00:23:46");

        // The stop ends the count and clears the display; the finalisation
        // keeps the meeting on the mark without a timer.
        feed(events, "meeting.state", {
            "state": "stopping",
            "meeting_id": root.meetingId,
            "duration_ms": 1500000
        });
        compare(DettivoState.meetingRecording, false);
        compare(DettivoState.meetingElapsed, "");
        compare(DettivoState.meetingClock.ticker.running, false);
        feed(events, "meeting.state", {
            "state": "transcribing",
            "meeting_id": root.meetingId
        });
        compare(DettivoState.meetingActive, true);
        compare(DettivoState.meetingRecording, false);
        compare(DettivoState.meetingElapsed, "");
        compare(glyph.item.elapsed, "");
        // The mark leaves the recording look while the transcript is
        // built, and the panel offers no Stop for a meeting past its stop.
        compare(DettivoState.glyphState, "transcribing");
        compare(glyph.item.state, "transcribing");
        compare(DettivoState.panelState, "transcribing");
        compare(widget.tooltip, "Dettivo — meeting transcribing");
        Quickshell.detached = [];
        DettivoState.stopMeeting();
        compare(DettivoState.pillState, "error");
        compare(DettivoState.meetingActions.process.running, false);
        DettivoState.pillHidden();
        feed(events, "meeting.state", {
            "state": "completed",
            "meeting_id": root.meetingId
        });
        compare(DettivoState.meetingActive, false);
        compare(DettivoState.meetingElapsed, "");
    }

    function test_2_a_reopened_stream_restores_the_anchor_from_started_at() {
        const events = Support.processFor(Quickshell, ["events", "--follow"]);
        verify(events);
        events.exit(2);
        compare(DettivoState.daemonAvailable, false);
        compare(DettivoState.meetingRecording, false);
        tryCompare(events, "running", true);
        const probe = Support.processFor(Quickshell, ["dictation", "status"]);
        tryCompare(probe, "running", true);
        probe.finish(JSON.stringify({
            "is_active": false
        }));
        const meetings = Support.processFor(Quickshell, ["meetings", "list"]);
        tryCompare(meetings, "running", true);
        DettivoState.meetingClock.pinnedNowMs = root.t0 + 754000;
        meetings.finish(JSON.stringify({
            "items": [
                {
                    "ref": {
                        "kind": "meeting",
                        "id": root.meetingId
                    },
                    "status": "recording",
                    "started_at": "2026-09-16T10:00:00Z",
                    "duration_seconds": 0
                }
            ]
        }));
        compare(DettivoState.meetingActive, true);
        compare(DettivoState.meetingRecording, true);
        compare(DettivoState.meetingId, root.meetingId);
        compare(DettivoState.meetingElapsed, "00:12:34");

        // A snapshot without the stamp keeps the anchor of the same meeting.
        DettivoState.refresh();
        tryCompare(meetings, "running", true);
        meetings.finish(JSON.stringify({
            "items": [
                {
                    "ref": {
                        "kind": "meeting",
                        "id": root.meetingId
                    },
                    "status": "recording"
                }
            ]
        }));
        compare(DettivoState.meetingElapsed, "00:12:34");
        DettivoState.meetingClock.pinnedNowMs = root.t0 + 755000;
        tryCompare(DettivoState, "meetingElapsed", "00:12:35");

        // The stream's exit ends the indication until a snapshot restores it.
        events.exit(2);
        compare(DettivoState.meetingRecording, false);
        compare(DettivoState.meetingElapsed, "");
        compare(DettivoState.meetingClock.ticker.running, false);
    }

    // The clock belongs to the meeting that records: the passes of an
    // older meeting publish their transitions under its own id and never
    // clear or rewind the new count; a reset that names no meeting still
    // clears it.
    function test_3_an_older_meetings_passes_never_touch_the_new_clock() {
        const older = "5eed0000-0000-4000-8000-00000000a003";
        const events = Support.processFor(Quickshell, ["events", "--follow"]);
        verify(events);
        feed(events, "meeting.state", {
            "state": "recording",
            "meeting_id": root.meetingId,
            "duration_ms": 5000
        });
        compare(DettivoState.meetingElapsed, "00:00:05");
        feed(events, "meeting.state", {
            "state": "completed",
            "meeting_id": older,
            "previous_state": "transcribing",
            "diarization_status": "queued",
            "analysis_status": "queued"
        });
        feed(events, "meeting.state", {
            "state": "completed",
            "meeting_id": older,
            "diarization_status": "ready"
        });
        feed(events, "meeting.state", {
            "state": "transcribing",
            "meeting_id": older,
            "duration_ms": 900000
        });
        compare(DettivoState.meetingRecording, true);
        compare(DettivoState.meetingId, root.meetingId);
        compare(DettivoState.meetingElapsed, "00:00:05");
        compare(DettivoState.meetingClock.ticker.running, true);
        DettivoState.meetingClock.pinnedNowMs = root.t0 + 1000;
        tryCompare(DettivoState, "meetingElapsed", "00:00:06");

        // The recording meeting's own stop still ends the count.
        feed(events, "meeting.state", {
            "state": "stopping",
            "meeting_id": root.meetingId
        });
        compare(DettivoState.meetingRecording, false);
        compare(DettivoState.meetingElapsed, "");

        // A reset without an id clears whatever records.
        feed(events, "meeting.state", {
            "state": "recording",
            "meeting_id": root.meetingId,
            "duration_ms": 0
        });
        compare(DettivoState.meetingRecording, true);
        DettivoState.setMeeting("completed", "", undefined);
        compare(DettivoState.meetingRecording, false);
        compare(DettivoState.meetingActive, false);
        compare(DettivoState.meetingElapsed, "");
    }
}
