import QtQuick

// What the meeting detail follows while it is open: a speaker renamed
// relabels every row, a pass started by hand is tracked by its job, a
// refusal lands in the header's outcome line (the rename and the
// retitle carry their own), a recovery started from Re-run reads the
// meeting again so the strip shows the transcription, an export names the file it wrote, and a
// render of the rename state opens the popover once the speakers are on
// screen.
Item {
    id: root

    // The MeetingDetail page: its `detail`, `meetingId`, `actions`,
    // `meetingsActions`, `actionNote`, `qaState` and `loaded`.
    property var page: null
    property bool qaRenameShown: false

    Connections {
        target: root.page ? root.page.detail : null
        function onChanged() {
            if (root.page.qaState === "rename" && root.page.loaded && !root.qaRenameShown) {
                root.qaRenameShown = true;
                Qt.callLater(() => root.page.renameSpeaker());
            }
        }
    }

    Connections {
        target: root.page ? root.page.meetingsActions : null
        function onRenamed(meetingId, speakerId, name, segmentsUpdated) {
            if (meetingId === root.page.meetingId && root.page.detail)
                root.page.detail.applyRename(speakerId, name);
        }
        function onAnalysisStarted(meetingId, jobId) {
            if (meetingId === root.page.meetingId && root.page.detail)
                root.page.detail.trackJob(jobId, "analyzing");
        }
        function onDiarizeStarted(meetingId, jobId) {
            if (meetingId === root.page.meetingId && root.page.detail)
                root.page.detail.trackJob(jobId, "diarizing");
        }
        function onRecovered(meetingId) {
            if (meetingId === root.page.meetingId && root.page.detail) {
                root.page.actionNote = "";
                root.page.detail.reload();
            }
        }
        function onFailed(action, reason) {
            if (action !== "rename" && action !== "retitle")
                root.page.actionNote = reason;
        }
    }

    Connections {
        target: root.page ? root.page.actions : null
        function onExported(path, bytes) {
            root.page.actionNote = qsTr("Wrote %1.").arg(path);
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Detail followers")
}
