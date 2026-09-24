import QtQuick
import QtTest
import Quickshell
import "plugin_support.js" as Support
import "fixtures.js" as Fixtures

// The real plugin under the shell shim: events drive the glyph, panel and
// pill; completion, failures, reconnection and level response stay visible.
TestCase {
    id: root
    name: "OmarchyPluginEvents"
    width: 480
    height: 520
    visible: true
    when: windowShown

    Component {
        id: widgetC
        BarWidget {}
    }

    Component {
        id: pillC
        DettivoPill {}
    }

    function feed(events, topic, payload) {
        events.feed(JSON.stringify({
            "topic": topic,
            "payload": payload
        }));
    }

    function test_1_the_event_stream_drives_glyph_panel_and_pill() {
        // The singleton is created on first use; touch it before its
        // processes are answered.
        compare(DettivoState.binary, "dettivo");
        Support.readyDaemon(Quickshell);
        Support.processFor(Quickshell, ["meetings", "list"]).finish(JSON.stringify({
            items: []
        }));
        compare(DettivoState.hint, "");
        compare(DettivoState.daemonAvailable, true);
        compare(DettivoState.panelState, "idle");
        compare(DettivoState.modeIndex, 2);
        compare(DettivoState.openShortcut, "SUPER+SHIFT+D");
        compare(DettivoState.engineFact, "parakeet-v3 · local");
        compare(DettivoState.enhancedFact, "qwen3-4b · loaded");
        compare(DettivoState.insertFact, "ghostty · virtual_keyboard");
        compare(DettivoState.recent.length, 2);
        compare(DettivoState.recent[0].title, "Add a regression test");
        compare(DettivoState.recent[0].app, "ghostty");
        compare(DettivoState.osdPosition, "bottom_right");

        const widget = createTemporaryObject(widgetC, root);
        const glyph = Support.glyphLoader(widget);
        tryCompare(glyph, "status", Loader.Ready);
        const button = Support.button(widget);
        const panel = Support.contentLoader(widget);
        tryCompare(panel, "status", Loader.Ready);
        compare(panel.item.state, "idle");
        compare(panel.item.engine, "parakeet-v3 · local");
        compare(panel.item.recent.length, 2);

        const events = Support.processFor(Quickshell, ["events", "--follow"]);
        verify(events);
        feed(events, "dictation.state", {
            "state": "recording"
        });
        compare(DettivoState.dictationState, "recording");
        compare(glyph.item.state, "listening");
        compare(button.active, true);
        compare(panel.item.state, "recording");
        compare(DettivoState.pillState, "listening");
        feed(events, "audio.level", {
            "rms": 0.6
        });
        fuzzyCompare(glyph.item.level, 0.6, 0.001);
        const pill = createTemporaryObject(pillC, root);
        feed(events, "audio.level", {
            "rms": 0.01
        });
        verify(pill.level > 0.25 && pill.level < 0.5, "quiet speech is visible");
        compare(DettivoState.level, 0.01, "display gain leaves the source level unchanged");
        const quiet = pill.level;
        feed(events, "audio.level", {
            "rms": 0.1
        });
        verify(pill.level > quiet && pill.level < 1, "normal speech retains headroom");
        feed(events, "audio.level", {
            "rms": 0.0001
        });
        compare(pill.level, 0, "background noise stays at the floor");
        feed(events, "audio.level", {
            "rms": 0
        });
        compare(pill.level, 0);
        feed(events, "audio.level", {
            "rms": 1
        });
        compare(pill.level, 1);
        feed(events, "engine.state", {
            "model": "parakeet-v3",
            "backend": "vulkan"
        });
        compare(DettivoState.engineLabel, "parakeet-v3 · vulkan");
        feed(events, "dictation.state", {
            "state": "transcribing"
        });
        compare(panel.item.state, "transcribing");
        compare(DettivoState.pillState, "transcribing");
        feed(events, "dictation.state", {
            "state": "inserting",
            "previous_state": "transcribing"
        });
        compare(DettivoState.pillState, "transcribing");
        const completion = Fixtures.completion.notification.params;
        compare(completion.topic, "dictation.state");
        compare(completion.payload.state, "idle");
        compare(completion.payload.previous_state, "inserting");
        feed(events, completion.topic, completion.payload);
        compare(panel.item.state, "inserted");
        compare(panel.item.sentence, "Inserted into ghostty");
        compare(DettivoState.pillState, "inserted");
        compare(DettivoState.pillTarget, "ghostty");
        compare(button.active, false);
        const history = Support.processFor(Quickshell, ["history", "list"]);
        compare(history.running, true);
        history.finish(JSON.stringify({
            "items": []
        }));
        compare(DettivoState.recent.length, 0);
        DettivoState.pillHidden();
        compare(panel.item.state, "idle");

        feed(events, "meeting.state", {
            "state": "recording",
            "live_last_end_ms": 1421000
        });
        compare(DettivoState.meetingActive, true);
        compare(DettivoState.meetingElapsed, "00:23:41");
        compare(glyph.item.state, "meeting");
        compare(glyph.item.elapsed, "23:41");
        compare(panel.item.sentence, "Meeting recording · 00:23:41");
        feed(events, "meeting.state", {
            "state": "completed"
        });
        compare(DettivoState.meetingActive, false);

        feed(events, "dictation.state", {
            "state": "failed",
            "previous_state": "recording",
            "reason": "no microphone"
        });
        compare(DettivoState.pillState, "error");
        compare(DettivoState.pillReason, "no microphone");
        compare(glyph.item.state, "error");
        // The daemon follows `failed` with `idle` at once; the pill keeps
        // the reason until its own timer hides it.
        feed(events, "dictation.state", {
            "state": "idle",
            "previous_state": "failed",
            "reason": "no microphone"
        });
        compare(DettivoState.pillState, "error");
        compare(glyph.item.state, "error");
        DettivoState.pillHidden();
        compare(DettivoState.dictationState, "idle");

        // A cancel hides the pill, and so does the idle that follows it.
        feed(events, "dictation.state", {
            "state": "recording",
            "previous_state": "idle"
        });
        compare(DettivoState.pillState, "listening");
        feed(events, "dictation.state", {
            "state": "cancelled",
            "previous_state": "recording",
            "reason": "cancelled"
        });
        compare(DettivoState.pillState, "hidden");
        feed(events, "dictation.state", {
            "state": "idle",
            "previous_state": "cancelled",
            "reason": "cancelled"
        });
        compare(DettivoState.pillState, "hidden");
        compare(DettivoState.dictationState, "idle");

        // The clipboard fallback rides on the same completion shape.
        const copied = JSON.parse(JSON.stringify(completion.payload));
        copied.insertion.outcome = "copied_to_clipboard";
        copied.insertion.method = "fallback_copy";
        copied.insertion.reason = "no text input focused";
        feed(events, completion.topic, copied);
        compare(DettivoState.pillState, "copied");
        compare(DettivoState.pillReason, "no text input focused");
        compare(DettivoState.pillAction, "Ctrl+V");
        compare(DettivoState.pillWords, "Add a regression test.");

        events.exit(2);
        compare(DettivoState.daemonAvailable, false);
        compare(panel.item.state, "unavailable");
        compare(DettivoState.pillState, "hidden");
        tryCompare(events, "running", true);

        // ops-and-record/F11: the stream reopened during a dictation and a
        // meeting; the snapshots restore both, with the meeting's id.
        const probe = Support.processFor(Quickshell, ["dictation", "status"]);
        tryCompare(probe, "running", true);
        probe.finish(JSON.stringify({
            "is_active": true,
            "job": {
                "job_id": "job_dict_7",
                "state": "running",
                "message": "listening with whisper/tiny.en"
            }
        }));
        compare(DettivoState.daemonAvailable, true);
        compare(DettivoState.dictationState, "recording");
        compare(panel.item.state, "recording");
        const meetings = Support.processFor(Quickshell, ["meetings", "list"]);
        verify(meetings);
        tryCompare(meetings, "running", true);
        meetings.finish(JSON.stringify({
            "items": [
                {
                    "ref": {
                        "kind": "meeting",
                        "id": "0f8fad5b-d9cb-469f-a165-70867728950e"
                    },
                    "status": "recording"
                }
            ]
        }));
        compare(DettivoState.meetingActive, true);
        compare(DettivoState.meetingId, "0f8fad5b-d9cb-469f-a165-70867728950e");

        // A snapshot older than an event never wins: the stream said the
        // dictation finished before the probe's answer arrived.
        DettivoState.refresh();
        tryCompare(probe, "running", true);
        feed(events, "dictation.state", {
            "state": "cancelled"
        });
        compare(DettivoState.dictationState, "idle");
        probe.finish(JSON.stringify({
            "is_active": true,
            "job": {
                "message": "listening"
            }
        }));
        compare(DettivoState.dictationState, "idle");
        feed(events, "meeting.state", {
            "state": "completed",
            "meeting_id": "0f8fad5b-d9cb-469f-a165-70867728950e"
        });
        compare(DettivoState.meetingActive, false);
        compare(DettivoState.meetingId, "");
    }

    function test_2_every_action_is_one_command() {
        // The singleton is created on first use; touch it before its
        // processes are answered.
        compare(DettivoState.binary, "dettivo");
        Support.readyDaemon(Quickshell);
        const widget = createTemporaryObject(widgetC, root);
        const panel = Support.contentLoader(widget);
        tryCompare(panel, "status", Loader.Ready);
        Quickshell.detached = [];
        panel.item.dictate();
        panel.item.stopDictation();
        panel.item.cancelDictation();
        panel.item.startMeeting();
        const meetingAction = Support.processFor(Quickshell, ["app", "open", "meetings"]);
        verify(meetingAction.running);
        meetingAction.finish("{}");
        DettivoState.setMeeting("recording", "0f8fad5b-d9cb-469f-a165-70867728950e", 0);
        panel.item.stopMeeting();
        compare(meetingAction.command, ["dettivo", "--json", "call", "meetings.stop", JSON.stringify({
                meeting_id: DettivoState.meetingId
            })]);
        meetingAction.finish("{}");
        panel.item.modeSelected(1);
        panel.item.openDettivo();
        panel.item.openItem(0);
        widget.settings = {
            "glyph": "dot",
            "levelMeter": false,
            "osd": "service"
        };
        const commands = Quickshell.detached.map(c => c.slice(2).join(" "));
        compare(commands, ["dictation start", "dictation stop", "dictation cancel", "config set dictation.mode polish", "app open home", "app open history", "config set omarchy.glyph dot", "config set omarchy.level_meter false", "config set omarchy.osd service"]);
        for (const c of Quickshell.detached)
            compare(c.slice(0, 2), ["dettivo", "--quiet"]);
        compare(DettivoState.modeIndex, 1);
    }
}
