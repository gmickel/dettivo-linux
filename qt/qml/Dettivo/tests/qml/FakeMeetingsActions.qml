import QtQuick

// The meetings actions for the tests: one meeting-capable provider, a
// remembered name, and every rename, delete, recover and discard
// recorded; a rename answers at once.
QtObject {
    property bool busy: false
    property string deletePolicy: "transcript_only"
    property var suggestions: [
        {
            "name": "Tobias",
            "uses": 3
        }
    ]
    property var providers: [
        {
            "id": "whisper",
            "label": "Whisper",
            "models": [
                {
                    "id": "small",
                    "label": "Small",
                    "downloaded": true
                }
            ]
        }
    ]
    property string selectedProvider: "whisper"
    property string selectedModel: "small"
    property bool importing: false
    property var suggested: []
    property var renames: []
    property var retitles: []
    property string refuseTitle: ""
    property var removed: []
    property var discarded: []
    property var recovered: []
    property var cancellations: []
    property var picked: []

    signal renamed(string meetingId, string speakerId, string name, int segmentsUpdated)
    signal retitled(string meetingId, string title)
    signal analysisStarted(string meetingId, string jobId)
    signal diarizeStarted(string meetingId, string jobId)
    signal importStarted(string meetingId, string jobId)
    signal importEnded(string meetingId, string stage)
    signal failed(string action, string reason)
    signal cancelled(string meetingId)

    function loadProviders() {
    }

    function pickEngine(providerId, modelId) {
        picked.push(providerId + ":" + modelId);
    }

    function suggest(prefix) {
        suggested.push(prefix);
    }

    function rename(meetingId, speakerId, name) {
        renames.push({
            "speakerId": speakerId,
            "name": name
        });
        renamed(meetingId, speakerId, name, 2);
    }

    function retitle(meetingId, title) {
        retitles.push(title);
        if (refuseTitle.length > 0)
            failed("retitle", refuseTitle);
        else
            retitled(meetingId, title);
    }

    function remove(meetingId, policy) {
        removed.push(meetingId);
    }

    function discard(id) {
        discarded.push(id);
    }

    function recover(id) {
        recovered.push(id);
    }

    function cancel(id) {
        cancellations.push(id);
    }

    function analyze(meetingId, force) {
    }

    function diarize(meetingId, speakers) {
    }

    function probeFile(path) {
        return {
            "ok": false,
            "reason": "Name an audio file to import."
        };
    }
}
