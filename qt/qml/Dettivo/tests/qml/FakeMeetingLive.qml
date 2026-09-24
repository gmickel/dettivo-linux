import QtQuick

// The live model for the tests: the facts the screens bind, every start
// recorded with its choices, the dismissals counted.
QtObject {
    property bool active: false
    property bool recording: false
    property bool finishing: false
    property bool starting: false
    property string state: "idle"
    property string meetingId: ""
    property string title: "Dettivo Linux kickoff"
    property string engineLabel: "whisper small"
    property bool systemAudio: true
    property string micDevice: "Arctis Nova"
    property string systemDevice: "monitor of Arctis Nova"
    property string error: ""
    property string elapsedText: "00:23:41"
    property real micLevel: 0.5
    property real micPeak: 0.7
    property real systemLevel: 0.2
    property real systemPeak: 0.3
    property int chunksDone: 0
    property int chunksTotal: 0
    property var segments: null
    property string notes: ""
    property string notesState: ""
    property bool disclosureAcknowledged: true
    property string disclosureAt: "11:04"
    property string disclosureMessage: "May be recorded."
    property var starts: []
    property int dismissed: 0

    signal disclosureRequired(string message)
    signal started(string meetingId)
    signal completed(string meetingId)
    signal failed(string action, string reason)
    signal changed

    function start(title, systemAudio, provider, model, speakers, analyze, diarize) {
        starts.push({
            "systemAudio": systemAudio,
            "provider": provider,
            "model": model,
            "speakers": speakers,
            "analyze": analyze,
            "diarize": diarize
        });
    }

    function dismissStart() {
        dismissed += 1;
    }

    function acknowledgeAndStart() {
    }

    function loadDisclosure() {
    }

    function copyDisclosure() {
    }
}
