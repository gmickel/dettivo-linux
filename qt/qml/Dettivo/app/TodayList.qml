pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Today as a plain history list (home.png): the time in tabular numerals,
// the title, the kind and the duration, one hairline between rows. The
// heading follows the newest day the daemon knows; an empty store shows
// the designed state instead of nothing.
Item {
    id: root

    property var model: null
    property var router: null
    // The Status model, for the hold chord the empty state names; without
    // one the default binding stands in.
    property var status: null
    property string heading: qsTr("Today")

    readonly property int count: root.model ? root.model.count : 0
    readonly property string hold: root.status && root.status.holdChord.length > 0 ? root.status.holdChord : "F9"

    function move(delta) {
        if (list.count === 0)
            return;
        const next = list.currentIndex < 0 ? (delta > 0 ? 0 : list.count - 1) : list.currentIndex + delta;
        list.currentIndex = Math.max(0, Math.min(list.count - 1, next));
        list.positionViewAtIndex(list.currentIndex, ListView.Contain);
        if (list.currentItem)
            list.currentItem.forceActiveFocus(Qt.TabFocusReason);
    }

    SectionHeading {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        title: root.heading
        trailing: qsTr("History")
    }

    ListView {
        id: list
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: 0
        clip: true
        currentIndex: -1
        model: root.model
        visible: root.count > 0

        delegate: ListRow {
            id: row
            required property var model
            required property int index
            readonly property bool meeting: row.model.kind === "meeting"
            horizontalPadding: 0
            leading: row.model.time
            text: row.model.title
            selected: row.activeFocus
            onActiveFocusChanged: {
                if (row.activeFocus)
                    list.currentIndex = row.index;
            }
            trailing: row.meeting ? row.model.duration : qsTr("%1 · %2").arg(row.model.kind).arg(row.model.duration)
            width: list.width
            onActivated: {
                if (root.router)
                    root.router.open(row.meeting ? "meetings.detail" : "history.detail", row.model.itemId);
            }

            trailingItem: Chip {
                text: qsTr("Meeting")
                visible: row.meeting
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Today's dictations")
    }

    StateView {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: Theme.space6
        keyHint: root.hold
        reason: qsTr("Hold %1 in any app and it will show up here.").arg(root.hold)
        title: qsTr("Nothing dictated yet.")
        visible: root.count === 0
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Today")
}
