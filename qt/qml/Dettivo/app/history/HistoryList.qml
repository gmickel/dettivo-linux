pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The list column of History: the heading with the item count, the
// search field with its hit count, the kind filter chips, the day labels
// and the rows, or the designed states for an empty store and a query
// with no match.
Item {
    id: root

    property var model: null
    // The HistoryModel behind `model`: the query, the hits, the error.
    property var search: null
    property string selectedId: ""
    property string hold: "F9"
    property alias currentIndex: list.currentIndex
    readonly property bool searchFocused: field.focused
    readonly property int count: root.model ? root.model.count : 0
    readonly property string query: root.search ? root.search.query : ""
    readonly property bool searching: root.search ? root.search.searching : false
    readonly property bool loaded: root.search ? root.search.loaded : true

    signal activated(string id)
    signal filterChosen(string filter)
    signal querySubmitted(string query)

    function focusSearch() {
        field.takeFocus();
    }

    function blurSearch() {
        field.dropFocus();
    }

    function idAt(index) {
        if (!root.model || index < 0 || index >= root.model.count)
            return "";
        const source = root.model.sourceRow ? root.model.sourceRow(index) : index;
        return root.search && root.search.idAt ? root.search.idAt(source) : "";
    }

    // The heading and the count on one line (history.png), so the search
    // field sits right under the title.
    Item {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.rowPaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.rowPaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.space4
        height: heading.implicitHeight

        Text {
            id: heading
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
            font.pixelSize: Theme.typeTitleSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("History")
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("History")
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            bottomPadding: 0
            rightPadding: 0
            text: root.searching ? "" : (root.count === 1 ? qsTr("1 item") : qsTr("%1 items").arg(root.count))
            topPadding: 0
        }
    }

    SearchField {
        id: field
        anchors.left: parent.left
        anchors.leftMargin: Theme.rowPaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.rowPaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space3
        hits: root.search ? root.search.hitCount : 0
        searching: root.searching
        onQueryChanged: query => root.querySubmitted(query)
    }

    FilterChips {
        id: chips
        anchors.left: parent.left
        anchors.leftMargin: Theme.rowPaddingX
        anchors.top: field.bottom
        anchors.topMargin: Theme.space3
        current: root.model && root.model.filter ? root.model.filter : "all"
        onChosen: filter => root.filterChosen(filter)
    }

    ListView {
        id: list
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: chips.bottom
        anchors.topMargin: Theme.space4
        clip: true
        currentIndex: -1
        model: root.model
        section.criteria: ViewSection.FullString
        section.delegate: SectionLabel {
            required property string section
            leftPadding: Theme.rowPaddingX
            text: section
            topPadding: Theme.space4
        }
        section.property: "dayHeading"
        visible: root.count > 0

        delegate: HistoryRow {
            id: row
            required property var model
            required property int index
            matches: row.model.matches
            meta: row.model.meta
            progress: row.model.progress
            selected: row.model.itemId === root.selectedId
            text: root.searching && row.model.snippet.length > 0 ? row.model.snippet : row.model.title
            time: row.model.time
            trailingChip: row.model.kind === "meeting" ? qsTr("Meeting") : ""
            width: list.width
            onActivated: {
                list.currentIndex = row.index;
                root.activated(row.model.itemId);
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Dictations")
    }

    StateView {
        anchors.left: parent.left
        anchors.leftMargin: Theme.rowPaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.rowPaddingX
        anchors.top: chips.bottom
        anchors.topMargin: Theme.space7
        keyHint: root.searching ? "" : root.hold
        reason: {
            if (root.searching)
                return root.search && root.search.error.length > 0 ? root.search.error : qsTr("Search covers dictations, meeting transcripts, notes and analysis.");
            return qsTr("Hold %1 in any app and it will show up here.").arg(root.hold);
        }
        title: root.searching ? qsTr("No match for \"%1\".").arg(root.query) : qsTr("Nothing dictated yet.")
        visible: root.count === 0 && root.loaded
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("History list")
}
