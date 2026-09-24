import QtQuick
import QtTest
import Quickshell
import "plugin_support.js" as Support

TestCase {
    name: "OmarchyPluginActions"
    when: windowShown

    readonly property string meetingId: "0f8fad5b-d9cb-469f-a165-70867728950e"

    function init() {
        compare(DettivoState.binary, "dettivo");
        Support.readyDaemon(Quickshell);
        DettivoState.retry.stop();
        DettivoState.pillHidden();
        DettivoState.setMeeting("recording", meetingId, 0);
        Quickshell.detached = [];
    }

    function test_stop_retains_the_id_and_observes_success_and_failure() {
        DettivoState.stopMeeting();
        const action = Support.processFor(Quickshell, ["call", "meetings.stop"]);
        verify(action, "meeting stop must be observed, not detached");
        compare(action.command, ["dettivo", "--json", "call", "meetings.stop", JSON.stringify({
                meeting_id: meetingId
            })]);
        action.finish(JSON.stringify({
            ref: {
                id: meetingId
            },
            job: {
                state: "succeeded"
            }
        }));
        const snapshot = Support.processFor(Quickshell, ["meetings", "list"]);
        verify(snapshot.running);
        snapshot.finish(JSON.stringify({
            items: []
        }));
        compare(DettivoState.meetingId, "");
        DettivoState.setMeeting("recording", meetingId, 0);
        DettivoState.stopMeeting();
        action.stdout.text = JSON.stringify({
            error: {
                message: "meeting not found"
            }
        });
        action.stdout.streamFinished();
        action.exit(4);
        compare(DettivoState.pillState, "error");
        compare(DettivoState.pillReason, "meeting not found");
    }

    function test_start_uses_the_apps_disclosure_flow_and_reports_launch_failure() {
        DettivoState.setMeeting("completed", "", undefined);
        DettivoState.startMeeting();
        const action = Support.processFor(Quickshell, ["app", "open", "meetings"]);
        verify(action, "start must open the app's existing disclosure/setup flow");
        compare(action.command, ["dettivo", "--json", "app", "open", "meetings"]);
        action.stderr.text = "app executable missing";
        action.stderr.streamFinished();
        action.exit(2);
        compare(DettivoState.pillReason, "app executable missing");
    }

    function test_disconnect_invalidates_pending_snapshot_and_stop_target() {
        DettivoState.refresh();
        const snapshot = Support.processFor(Quickshell, ["meetings", "list"]);
        const events = Support.processFor(Quickshell, ["events", "--follow"]);
        events.exit(2);
        DettivoState.retry.stop();
        DettivoState.refresh();
        snapshot.finish(JSON.stringify({
            items: [
                {
                    ref: {
                        id: meetingId
                    },
                    status: "recording"
                }
            ]
        }));
        compare(DettivoState.meetingId, "", "a pre-disconnect snapshot must not restore a stale stop target");
        DettivoState.stopMeeting();
        compare(Quickshell.detached.length, 0);
        compare(DettivoState.pillState, "error");
    }
}
