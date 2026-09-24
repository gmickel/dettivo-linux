import QtQuick

// The meetings list for the tests: one analysed row this week, with the
// list model's properties and functions the route reads.
ListModel {
    property bool loaded: true
    property string query: ""
    property bool searching: false
    property int hitCount: 0
    property string error: ""
    property string qaState: ""
    property int refreshes: 0
    property var retitles: []

    function search(query) {
    }

    function idAt(row) {
        return row === 0 ? "m1" : "";
    }

    function rowOf(id) {
        return id === "m1" ? 0 : -1;
    }

    function refresh() {
        refreshes += 1;
    }

    function remove(id) {
    }

    function retitle(id, title) {
        retitles.push(id + ":" + title);
        if (rowOf(id) >= 0)
            setProperty(rowOf(id), "title", title);
    }

    ListElement {
        meetingId: "m1"
        title: "Dettivo Linux kickoff"
        summary: "Engine processes, one protocol"
        when: "Wed 11:05"
        date: "3 Sep"
        week: "This week"
        length: "41 min"
        status: "completed"
        partial: false
        recoverable: false
        hasNotes: true
        analysisStatus: "ready"
        speakerCount: 3
        chip: "Analysed"
        chipKind: "analysed"
        snippet: ""
        matchedField: ""
    }
}
