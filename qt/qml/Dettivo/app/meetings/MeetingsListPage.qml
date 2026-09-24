pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The meetings list beside the new-meeting rail (meetings-list.png): the
// week labels and the rows on the left, the rail with the sources, the
// engine, the speakers, the analysis, the disclosure and Start meeting on
// the right, and the footer with the keys under the list.
Item {
    id: root

    property var router: null
    property var status: null
    property var meetings: null
    property var live: null
    property var actions: null
    property var config: null
    property string selectedId: ""
    property string notice: ""

    readonly property bool typing: list.searchFocused
    readonly property int currentIndex: list.currentIndex

    signal activated(string id)
    signal importRequested

    function focusSearch() {
        list.focusSearch();
    }

    function dismiss() {
        if (list.searchFocused) {
            list.blurSearch();
            return true;
        }
        return false;
    }

    function startMeeting() {
        rail.start();
    }

    function move(delta) {
        if (!root.meetings || root.meetings.count === 0)
            return;
        const next = Math.max(0, Math.min(root.meetings.count - 1, list.currentIndex + delta));
        list.currentIndex = next;
    }

    function openCurrent() {
        const id = root.meetings && root.meetings.idAt ? root.meetings.idAt(list.currentIndex) : "";
        if (id.length > 0)
            root.activated(id);
    }

    MeetingsList {
        id: list
        actions: root.actions
        anchors.bottom: footer.top
        anchors.left: parent.left
        anchors.right: rail.left
        anchors.top: parent.top
        meetings: root.meetings
        notice: root.notice
        selectedId: root.selectedId
        onActivated: id => root.activated(id)
        onImportRequested: root.importRequested()
    }

    MeetingsFooter {
        id: footer
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: rail.left
        hints: qsTr("j / k move · enter open · n new meeting · i import · / search")
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.right: rail.left
        anchors.top: parent.top
        color: Theme.roleHairline
        width: Theme.hairlineWidth
    }

    NewMeetingRail {
        id: rail
        actions: root.actions
        config: root.config
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.top: parent.top
        live: root.live
        router: root.router
        status: root.status
        width: Theme.rightRailWidth
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meetings page")
}
