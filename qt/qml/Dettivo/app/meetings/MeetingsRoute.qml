pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Meetings (meetings-list.png, meeting-live.png, meeting-detail.png): the
// list by week beside the new-meeting rail, the live meeting while one
// records, and the detail after the stop. A row opens `meetings.detail`
// with its id, `n` starts a meeting through the rail (the disclosure
// dialog first on a fresh profile), `i` imports a file, Escape returns to
// the list, and a completed meeting lands on its detail. The host hands
// the models and the actions in; the tests hand in fakes.
Item {
    id: root

    property var router: null
    property var status: null
    property var meetings: null
    property var live: null
    property var detail: null
    property var actions: null
    property var meetingsActions: null
    property var config: null

    readonly property string sub: root.router ? root.router.sub : ""
    readonly property string selectedId: root.router && root.sub === "detail" ? root.router.arg : ""
    readonly property string qaState: root.meetings && root.meetings.qaState ? root.meetings.qaState : ""
    readonly property var pageItem: pages.item
    property string notice: ""

    function focusSearch() {
        if (root.sub.length > 0 && root.router)
            root.router.back();
        if (root.pageItem && root.pageItem.focusSearch)
            root.pageItem.focusSearch();
        return true;
    }

    function dismiss() {
        if (importDialog.visible) {
            importDialog.close();
            return true;
        }
        if (disclosureDialog.visible) {
            disclosureDialog.dismiss();
            return true;
        }
        if (root.pageItem && root.pageItem.dismiss && root.pageItem.dismiss())
            return true;
        return false;
    }

    function open(id) {
        if (root.router && id.length > 0)
            root.router.open("meetings.detail", id);
    }

    function newMeeting() {
        if (root.sub.length > 0 && root.router)
            root.router.back();
        if (root.pageItem && root.pageItem.startMeeting)
            root.pageItem.startMeeting();
    }

    function importAudio() {
        importDialog.openFor("");
    }

    onSelectedIdChanged: {
        if (root.detail)
            root.detail.load(root.selectedId);
    }

    Component.onCompleted: {
        if (root.detail && root.selectedId.length > 0)
            root.detail.load(root.selectedId);
        if (root.live && root.live.loadDisclosure)
            root.live.loadDisclosure();
        if (root.meetingsActions && root.meetingsActions.loadProviders)
            root.meetingsActions.loadProviders();
        if (root.qaState === "import")
            importDialog.openSample();
        else if (root.qaState === "disclosure")
            disclosureDialog.openWith(root.live ? root.live.disclosureMessage : "");
    }

    Connections {
        target: root.live
        function onDisclosureRequired(message) {
            disclosureDialog.openWith(message);
        }
        function onStarted(meetingId) {
            root.notice = "";
            if (root.router)
                root.router.open("meetings.live");
        }
        function onCompleted(meetingId) {
            if (root.router && (root.sub === "live" || root.sub.length === 0))
                root.router.open("meetings.detail", meetingId);
            if (root.meetings)
                root.meetings.refresh();
        }
        function onFailed(action, reason) {
            root.notice = reason;
        }
    }

    Connections {
        target: root.meetingsActions
        function onRemoved(meetingId) {
            root.forget(meetingId);
        }
        function onDiscarded(meetingId) {
            root.forget(meetingId);
        }
        function onRecovered(meetingId) {
            if (root.meetings)
                root.meetings.refresh();
        }
        function onCancelled(meetingId) {
            root.notice = "";
            if (root.meetings)
                root.meetings.refresh();
        }
        function onRetitled(meetingId, title) {
            if (root.meetings && root.meetings.retitle)
                root.meetings.retitle(meetingId, title);
        }
        function onImportStarted(meetingId, jobId) {
            if (root.meetings)
                root.meetings.refresh();
        }
        function onImportEnded(meetingId, stage) {
            if (root.meetings)
                root.meetings.refresh();
            if (stage === "done" && root.router && root.sub.length === 0)
                root.router.open("meetings.detail", meetingId);
        }
        function onFailed(action, reason) {
            if (action !== "import" && action !== "rename" && action !== "retitle")
                root.notice = reason;
        }
    }

    function forget(meetingId) {
        if (root.meetings)
            root.meetings.remove(meetingId);
        if (meetingId === root.selectedId && root.router)
            root.router.back();
    }

    Loader {
        id: pages
        anchors.fill: parent
        sourceComponent: root.sub === "live" ? livePage : (root.sub === "detail" ? detailPage : listPage)
    }

    Component {
        id: listPage
        MeetingsListPage {
            actions: root.meetingsActions
            config: root.config
            live: root.live
            meetings: root.meetings
            notice: root.notice
            router: root.router
            selectedId: root.selectedId
            status: root.status
            onActivated: id => root.open(id)
            onImportRequested: root.importAudio()
        }
    }

    Component {
        id: livePage
        MeetingLive {
            live: root.live
            notice: root.notice
            router: root.router
        }
    }

    Component {
        id: detailPage
        MeetingDetail {
            actions: root.actions
            detail: root.detail
            meetingId: root.selectedId
            meetingsActions: root.meetingsActions
            qaState: root.qaState
            router: root.router
        }
    }

    MeetingsKeys {
        page: root.pageItem
        route: root
    }

    ImportDialog {
        id: importDialog
        actions: root.meetingsActions
        anchors.centerIn: parent
    }

    DisclosureDialog {
        id: disclosureDialog
        anchors.centerIn: parent
        live: root.live
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.sub === "live" ? qsTr("Meeting live") : (root.sub === "detail" ? qsTr("Meeting") : qsTr("Meetings"))
}
