import QtQuick
import QtQuick.Controls
import QtTest
import Dettivo

TestCase {
    id: root
    name: "MeetingRecovery"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    Component {
        id: rowC
        MeetingRow {
            width: 1000
            meetingId: "m1"
        }
    }

    function test_child_action_does_not_open_row_data() {
        return [
            {
                tag: "cancel",
                name: "Cancel transcription",
                calls: "cancellations",
                flags: {
                    cancellable: true
                }
            },
            {
                tag: "recover",
                name: "Recover",
                calls: "recovered",
                flags: {
                    recoverable: true
                }
            },
            {
                tag: "discard",
                name: "Discard",
                calls: "discarded",
                flags: {
                    partial: true
                }
            }
        ];
    }

    function test_child_action_does_not_open_row(data) {
        const actions = createTemporaryObject(actionsC, root);
        const properties = Object.assign({
            actions: actions
        }, data.flags);
        const row = createTemporaryObject(rowC, root, properties);
        let opened = 0;
        row.activated.connect(() => opened++);
        waitForRendering(row);
        mouseClick(named(row, data.name));
        compare(actions[data.calls].length, 1);
        compare(actions[data.calls][0], "m1");
        compare(opened, 0, "an inline action must not navigate into the meeting");
    }

    Component {
        id: routeC
        MeetingsRoute {
            width: 1200
            height: 750
        }
    }

    function test_cancel_waits_for_success_and_surfaces_failure() {
        const model = createTemporaryObject(modelC, root);
        model.setProperty(0, "status", "transcribing");
        const actions = createTemporaryObject(actionsC, root);
        const route = createTemporaryObject(routeC, root, {
            meetings: model,
            meetingsActions: actions
        });
        waitForRendering(route);
        const cancel = named(route, "Cancel transcription");
        verify(cancel);
        verify(cancel.visible);
        const refreshes = model.refreshes;
        mouseClick(cancel);
        compare(actions.cancellations.length, 1);
        compare(actions.cancellations[0], "m1");
        compare(model.get(0).status, "transcribing");
        actions.failed("cancel", "Cancellation refused");
        compare(route.notice, "Cancellation refused");
        compare(model.refreshes, refreshes);
        actions.cancelled("m1");
        compare(model.refreshes, refreshes + 1);
        compare(route.notice, "");
        model.setProperty(0, "status", "cancelled");
        model.setProperty(0, "recoverable", true);
        tryCompare(cancel, "visible", false);
        verify(named(route, "Recover").visible);
    }

    Component {
        id: listC
        MeetingsList {
            width: 1000
            height: 750
        }
    }
    Component {
        id: modelC
        FakeMeetingsModel {}
    }
    Component {
        id: actionsC
        FakeMeetingsActions {}
    }

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

    function test_recovery_uses_validated_audio_membership_data() {
        return [
            {
                tag: "failed-audio",
                status: "failed",
                recoverable: true,
                partial: false
            },
            {
                tag: "cancelled-audio",
                status: "cancelled",
                recoverable: true,
                partial: false
            },
            {
                tag: "failed-no-audio",
                status: "failed",
                recoverable: false,
                partial: false
            },
            {
                tag: "partial-audio",
                status: "partial",
                recoverable: true,
                partial: true
            },
            {
                tag: "partial-no-audio",
                status: "partial",
                recoverable: false,
                partial: true
            }
        ];
    }

    function test_recovery_uses_validated_audio_membership(data) {
        const model = createTemporaryObject(modelC, root);
        model.setProperty(0, "status", data.status);
        model.setProperty(0, "partial", data.partial);
        model.setProperty(0, "recoverable", data.recoverable);
        model.setProperty(0, "chip", data.status);
        const actions = createTemporaryObject(actionsC, root);
        const list = createTemporaryObject(listC, root, {
            meetings: model,
            actions: actions
        });
        let opened = 0;
        list.activated.connect(() => opened++);
        waitForRendering(list);
        const recover = named(list, "Recover");
        const discard = named(list, "Discard");
        verify(recover);
        verify(discard);
        compare(recover.visible, data.recoverable);
        compare(discard.visible, data.partial);
        verify(named(list, data.status));
        if (data.recoverable) {
            mouseClick(recover);
            compare(actions.recovered.length, 1);
            compare(actions.recovered[0], "m1");
            compare(actions.discarded.length, 0);
        }
        if (data.partial) {
            mouseClick(discard);
            compare(actions.discarded.length, 1);
            compare(actions.discarded[0], "m1");
        }
        compare(opened, 0);
    }

    Component {
        id: headerC
        DetailHeader {
            width: 1000
            meetingId: "m1"
        }
    }
    Component {
        id: detailC
        FakeMeetingDetail {}
    }

    // fn-76: Re-run on the meeting detail retries a meeting the daemon can
    // recover through `meetings.recover`; otherwise it stays disabled and
    // its description carries the reason the tooltip shows.
    function test_detail_rerun_recovers_a_retryable_meeting_data() {
        return [
            {
                tag: "failed-with-audio",
                status: "failed",
                canRecover: true,
                reason: "Transcribe the meeting again from its retained audio."
            },
            {
                tag: "failed-audio-gone",
                status: "failed",
                canRecover: false,
                reason: "The meeting's audio is gone, so it cannot be transcribed again."
            },
            {
                tag: "recording",
                status: "recording",
                canRecover: false,
                reason: "The meeting is still recording."
            },
            {
                tag: "completed",
                status: "completed",
                canRecover: false,
                reason: "A meeting re-run is reserved in the contract; import the audio again to transcribe it with another engine."
            }
        ];
    }

    function test_detail_rerun_recovers_a_retryable_meeting(data) {
        const detail = createTemporaryObject(detailC, root, {
            status: data.status,
            canRecover: data.canRecover,
            rerunReason: data.reason
        });
        const actions = createTemporaryObject(actionsC, root);
        const header = createTemporaryObject(headerC, root, {
            detail: detail,
            meetingsActions: actions
        });
        waitForRendering(header);
        const rerun = named(header, "Re-run");
        verify(rerun);
        compare(rerun.enabled, data.canRecover);
        compare(rerun.Accessible.description, data.reason);
        compare(rerun.ToolTip.text, data.reason);
        mouseMove(rerun);
        tryVerify(() => rerun.ToolTip.visible, 1000, "the reason shows on hover, enabled or not");
        mouseClick(rerun);
        compare(actions.recovered.length, data.canRecover ? 1 : 0);
        if (data.canRecover)
            compare(actions.recovered[0], "m1");
        actions.busy = true;
        compare(rerun.enabled, false);
    }
}
