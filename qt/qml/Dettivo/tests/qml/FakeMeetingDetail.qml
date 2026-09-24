import QtQuick

// The meeting detail for the tests: the analysed kickoff with two
// speakers and two segments, every rename applied recorded.
QtObject {
    property string meetingId: "m1"
    property bool loaded: true
    property bool loading: false
    property string error: ""
    property string title: "Dettivo Linux kickoff"
    property string factsLine: "Wed 3 Sep · 11:05 to 11:46 · 41 min"
    property string status: "completed"
    property bool partial: false
    property string partialLine: ""
    property bool hasPolished: true
    property string notes: "## Decisions"
    property string notesMeta: "1 section"
    property string notesState: ""
    property string analysisStatus: "ready"
    property string summary: "The kickoff agreed the engine model."
    property var decisions: ["Engine processes"]
    property var actionItems: ["You: write the spec"]
    property string analysisMeta: "qwen3 · 11:47"
    property string analysisError: ""
    property string diarizationStatus: "ready"
    property string diarizationLine: "speakers assigned"
    property string audioFacts: "2 tracks · 41 min · kept"
    property string exportFacts: "md · txt · srt · vtt · json"
    property real progress: -1
    property string stage: ""
    property string lastTab: "transcript"
    property string processingState: "done"
    property string processingLine: "Processing complete"
    property var stages: [
        {
            "key": "transcript",
            "label": "Transcript",
            "state": "done",
            "detail": "2 segments",
            "progress": -1
        },
        {
            "key": "speakers",
            "label": "Speakers",
            "state": "done",
            "detail": "2 speakers",
            "progress": -1
        },
        {
            "key": "analysis",
            "label": "Analysis",
            "state": "done",
            "detail": "qwen3 · 11:47",
            "progress": -1
        }
    ]
    property var renamed: []
    property var titles: []
    property var speakers: [
        {
            "speakerId": "you",
            "name": "You",
            "colorIndex": 0,
            "talkMs": 1442000,
            "talk": "24:02",
            "share": 0.58,
            "shareText": "58 %"
        },
        {
            "speakerId": "speaker_00",
            "name": "Mara",
            "colorIndex": 1,
            "talkMs": 670000,
            "talk": "11:10",
            "share": 0.42,
            "shareText": "42 %"
        }
    ]
    property var segments: [
        {
            "time": "11:05",
            "speaker": "You",
            "speakerId": "you",
            "colorIndex": 0,
            "text": "um lets lock",
            "polished": "Let's lock.",
            "gapBefore": 0,
            "startMs": 0
        },
        {
            "time": "11:19",
            "speaker": "Mara",
            "speakerId": "speaker_00",
            "colorIndex": 1,
            "text": "fine by me",
            "polished": "Fine by me.",
            "gapBefore": 0,
            "startMs": 840000
        }
    ]

    signal changed

    function speakerAt(index) {
        return index >= 0 && index < speakers.length ? speakers[index] : ({});
    }

    function applyRename(speakerId, name) {
        renamed.push(speakerId + ":" + name);
    }

    function applyTitle(name) {
        titles.push(name);
        title = name;
    }

    function trackJob(jobId, stage) {
    }

    function setNotes(markdown) {
        notes = markdown;
    }

    function load(id) {
    }
}
