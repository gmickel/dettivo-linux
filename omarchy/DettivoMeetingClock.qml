import QtQuick
import "DettivoText.js" as DettivoText

// The meeting timer the glyph, the panel and the tooltip show: the wall
// clock against the moment the recording started, ticking once a second
// on its own, so a quiet room and a daemon that reports no progress
// never freeze it (ADR 0063). The anchor comes from each `meeting.state`
// event's capture so far and from the snapshot's `started_at` when the
// stream reopens. Leaving `recording` stops the tick and clears the
// display, so a stopped meeting never shows a stuck count. Owned by
// DettivoState, which is `state` here.
QtObject {
    id: root

    required property var state
    // Epoch milliseconds the recording started at; 0 while none records.
    property real startedAtMs: 0
    // A test pins the clock here; below zero reads the wall clock.
    property real pinnedNowMs: -1

    function now() {
        return root.pinnedNowMs >= 0 ? root.pinnedNowMs : Date.now();
    }

    // `elapsedMs` is what the meeting has recorded so far when the caller
    // knows it; unknown keeps the anchor of the same meeting and starts a
    // new meeting's count from now. The clock belongs to the meeting that
    // records: another meeting's later transitions (its speaker or
    // analysis pass) never touch it, while a state that names no meeting
    // (the reset after a reconnect or an empty snapshot) always clears.
    function apply(meetingState, id, elapsedMs) {
        const recording = meetingState === "recording";
        const previousId = root.state.meetingId;
        const named = String(id || "");
        if (!recording && root.state.meetingRecording && named !== "" && named !== previousId)
            return;
        root.state.meetingActive = recording || meetingState === "transcribing";
        root.state.meetingId = root.state.meetingActive ? String(id || "") : "";
        if (!recording) {
            root.startedAtMs = 0;
            root.state.meetingRecording = false;
            root.state.meetingElapsed = "";
            return;
        }
        const known = Number(elapsedMs);
        if (elapsedMs !== undefined && elapsedMs !== null && !isNaN(known))
            root.startedAtMs = root.now() - Math.max(0, known);
        else if (root.startedAtMs === 0 || root.state.meetingId !== previousId)
            root.startedAtMs = root.now();
        root.state.meetingRecording = true;
        root.tick();
    }

    function tick() {
        if (root.state.meetingRecording && root.startedAtMs > 0)
            root.state.meetingElapsed = DettivoText.formatElapsed(root.now() - root.startedAtMs);
    }

    readonly property Timer ticker: Timer {
        interval: 1000
        repeat: true
        running: root.state.meetingRecording
        onTriggered: root.tick()
    }
}
