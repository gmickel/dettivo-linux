import QtQuick

// The history actions for the meetings tests: the export path recorded
// with its format and the raw switch.
QtObject {
    property bool busy: false
    property string exportDir: ""
    property var exports: []

    signal exported(string path, int bytes)
    signal failed(string action, string reason)

    function exportMeeting(id, format, raw, notesOverride) {
        exports.push({
            "id": id,
            "format": format,
            "raw": raw,
            "notesOverride": notesOverride
        });
    }
}
