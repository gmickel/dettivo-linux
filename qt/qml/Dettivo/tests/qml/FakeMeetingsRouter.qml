import QtQuick

// A router for the meetings tests: the route, the sub-route and the
// argument as plain properties, every `open` recorded.
QtObject {
    property string route: "meetings"
    property string sub: ""
    property string arg: ""
    property bool detail: sub.length > 0
    property var opened: []
    readonly property string page: sub.length > 0 ? route + "." + sub : route
    readonly property string title: route

    function open(name, id) {
        opened.push(id ? name + ":" + id : name);
        return true;
    }

    function back() {
        sub = "";
        arg = "";
    }
}
