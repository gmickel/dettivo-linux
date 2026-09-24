pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// History (history.png): the list column with search, the kind filters
// and the days, beside the dictation detail. A row opens `history.detail`
// with its id, Escape returns to the list, `/` focuses search, `j` and
// `k` move, Enter opens, `r`, `e` and `d` run the detail's actions. The
// host hands the models and the actions in; the tests hand in fakes.
Item {
    id: root

    property var router: null
    property var status: null
    property var history: null
    property var historyFiltered: null
    property var detail: null
    property var actions: null
    property var player: null

    readonly property string sub: root.router ? root.router.sub : ""
    readonly property string selectedId: root.router && root.sub === "detail" ? root.router.arg : ""
    readonly property string hold: root.status && root.status.holdChord.length > 0 ? root.status.holdChord : "F9"
    readonly property var listModel: root.historyFiltered ? root.historyFiltered : root.history

    // The window's `/` lands here: the search field takes focus.
    function focusSearch() {
        list.focusSearch();
        return true;
    }

    // The window's Escape: a focused search field gives focus back to the
    // list first; the router handles the detail after that.
    function dismiss() {
        if (list.searchFocused) {
            list.blurSearch();
            return true;
        }
        return false;
    }

    function open(id) {
        if (root.router && id.length > 0)
            root.router.open("history.detail", id);
    }

    function move(delta) {
        const model = root.listModel;
        if (!model || model.count === 0)
            return;
        const current = model.rowOf ? model.rowOf(root.selectedId) : -1;
        const next = Math.max(0, Math.min(model.count - 1, current + delta));
        list.currentIndex = next;
        const id = list.idAt(next);
        if (id.length > 0)
            root.open(id);
    }

    onSelectedIdChanged: {
        if (root.detail)
            root.detail.load(root.selectedId);
    }

    Component.onCompleted: {
        if (root.detail && root.selectedId.length > 0)
            root.detail.load(root.selectedId);
    }

    Connections {
        target: root.actions
        function onRerunStarted(itemId, newItemId, jobId) {
            if (root.history)
                root.history.refresh();
            if (root.history && root.history.trackJob)
                root.history.trackJob(jobId, newItemId);
            if (root.detail && itemId === root.selectedId)
                root.detail.trackJob(jobId);
        }
        function onRemoved(id) {
            if (root.history)
                root.history.remove(id);
            if (id === root.selectedId && root.router)
                root.router.back();
        }
    }

    HistoryList {
        id: list
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.top: parent.top
        hold: root.hold
        model: root.listModel
        search: root.history
        selectedId: root.selectedId
        width: Theme.historyListWidth
        onActivated: id => root.open(id)
        onFilterChosen: filter => {
            if (root.historyFiltered)
                root.historyFiltered.filter = filter;
        }
        onQuerySubmitted: query => {
            if (root.history)
                root.history.search(query);
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: list.right
        anchors.top: parent.top
        color: Theme.roleHairline
        width: Theme.hairlineWidth
    }

    HistoryDetail {
        id: detailPane
        actions: root.actions
        anchors.bottom: parent.bottom
        anchors.left: list.right
        anchors.leftMargin: Theme.hairlineWidth
        anchors.right: parent.right
        anchors.top: parent.top
        detail: root.detail
        itemId: root.selectedId
        player: root.player
    }

    Shortcut {
        enabled: !list.searchFocused
        sequence: "J"
        onActivated: root.move(1)
    }

    Shortcut {
        enabled: !list.searchFocused
        sequence: "K"
        onActivated: root.move(-1)
    }

    Shortcut {
        enabled: !list.searchFocused
        sequence: "Return"
        onActivated: {
            const id = list.idAt(list.currentIndex);
            if (id.length > 0)
                root.open(id);
        }
    }

    Shortcut {
        enabled: !list.searchFocused && root.selectedId.length > 0
        sequence: "R"
        onActivated: detailPane.rerun()
    }

    Shortcut {
        enabled: !list.searchFocused && root.selectedId.length > 0
        sequence: "E"
        onActivated: detailPane.exportSheet()
    }

    Shortcut {
        enabled: !list.searchFocused && root.selectedId.length > 0
        sequence: "D"
        onActivated: detailPane.confirmDelete()
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.sub === "detail" ? qsTr("Dictation") : qsTr("History")
}
