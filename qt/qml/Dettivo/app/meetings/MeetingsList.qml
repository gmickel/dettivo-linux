pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// The list column of Meetings (meetings-list.png): the heading with the
// search field and Import audio, the column labels, the weeks as section
// labels and one row per meeting, or the designed state for an empty
// store and a query with no match.
Item {
    id: root

    property var meetings: null
    property var actions: null
    property string selectedId: ""
    property string notice: ""
    property alias currentIndex: list.currentIndex
    readonly property bool searchFocused: search.focused
    readonly property int count: root.meetings ? root.meetings.count : 0
    readonly property bool searching: root.meetings ? root.meetings.searching : false
    readonly property bool loaded: root.meetings ? root.meetings.loaded : true
    readonly property string query: root.meetings ? root.meetings.query : ""
    readonly property int whenWidth: Theme.space8 * 2 + Theme.space7 + Theme.space2
    readonly property int lengthWidth: Theme.space8 * 2 + Theme.space3
    readonly property int speakersWidth: Theme.space8 * 3 + Theme.space6
    readonly property int stateWidth: Theme.space8 * 2 + Theme.space4

    signal activated(string id)
    signal importRequested

    function focusSearch() {
        search.takeFocus();
    }

    function blurSearch() {
        search.dropFocus();
    }

    Item {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.space4
        readonly property bool compact: width < heading.implicitWidth + Theme.space4 + Theme.space8 * 5 + Theme.space2 + importButton.width
        height: compact ? Theme.controlHeight * 2 + Theme.space2 : Theme.controlHeight

        Text {
            id: heading
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: header.compact ? -(Theme.controlHeight + Theme.space2) / 2 : 0
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
            font.pixelSize: Theme.typeTitleSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Meetings")
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("Meetings")
        }

        Button {
            id: importButton
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: header.compact ? (Theme.controlHeight + Theme.space2) / 2 : 0
            text: qsTr("Import audio")
            onClicked: root.importRequested()
        }

        SearchField {
            id: search
            placeholder: qsTr("Search meetings")
            fieldName: qsTr("Search meetings")
            clearName: qsTr("Clear meetings search")
            containerName: qsTr("Meetings search")
            anchors.right: importButton.left
            anchors.rightMargin: Theme.space2
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: header.compact ? (Theme.controlHeight + Theme.space2) / 2 : 0
            hits: root.meetings ? root.meetings.hitCount : 0
            searching: root.searching
            width: header.compact ? header.width - importButton.width - Theme.space2 : Theme.space8 * 5
            onQueryChanged: query => {
                if (root.meetings)
                    root.meetings.search(query);
            }
        }
    }

    FocusFlickable {
        id: viewport
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space4
        anchors.bottom: parent.bottom
        contentWidth: Math.max(width, Theme.appWindowWidth - Theme.sidebarWidth - Theme.rightRailWidth - Theme.pagePaddingX * 2)
        contentHeight: height
        flickableDirection: Flickable.HorizontalFlick
        activeFocusOnTab: contentWidth > width
        Accessible.name: qsTr("Meetings table")
        Keys.onLeftPressed: contentX = Math.max(0, contentX - Theme.space8)
        Keys.onRightPressed: contentX = Math.min(contentWidth - width, contentX + Theme.space8)
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Home)
                contentX = 0;
            else if (event.key === Qt.Key_End)
                contentX = contentWidth - width;
            else
                return;
            event.accepted = true;
        }

        FocusRing {
            objectName: "meetingsTableFocusRing"
            parent: viewport
        }

        Item {
            width: viewport.contentWidth
            height: viewport.height

            Row {
                id: columns
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                spacing: 0

                SectionLabel {
                    leftPadding: 0
                    text: qsTr("When")
                    width: root.whenWidth
                }

                SectionLabel {
                    leftPadding: 0
                    text: qsTr("Title")
                    width: columns.width - root.whenWidth - root.lengthWidth - root.speakersWidth - root.stateWidth
                }

                SectionLabel {
                    leftPadding: 0
                    text: qsTr("Length")
                    width: root.lengthWidth
                }

                SectionLabel {
                    leftPadding: 0
                    text: qsTr("Speakers")
                    width: root.speakersWidth
                }

                SectionLabel {
                    leftPadding: 0
                    text: qsTr("State")
                    width: root.stateWidth
                }

                Accessible.role: Accessible.Row
                Accessible.name: qsTr("Columns")
            }

            Text {
                id: noticeText
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: columns.bottom
                anchors.topMargin: root.notice.length > 0 ? Theme.space3 : 0
                color: Theme.roleUrgent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                height: root.notice.length > 0 ? implicitHeight : 0
                text: root.notice
                visible: root.notice.length > 0
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Meetings notice")
            }

            ListView {
                id: list
                anchors.bottom: parent.bottom
                anchors.bottomMargin: viewport.contentWidth > viewport.width ? Theme.space3 : 0
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: noticeText.bottom
                anchors.topMargin: Theme.space2
                clip: true
                currentIndex: -1
                model: root.meetings
                section.criteria: ViewSection.FullString
                section.delegate: SectionLabel {
                    required property string section
                    bottomPadding: Theme.space3
                    leftPadding: 0
                    text: section
                    topPadding: Theme.space5
                }
                section.property: "week"
                visible: root.count > 0

                delegate: MeetingRow {
                    id: row
                    required property var model
                    required property int index
                    actions: root.actions
                    cancellable: row.model.status === "transcribing"
                    chip: row.model.chip
                    chipKind: row.model.chipKind
                    date: row.model.date
                    length: row.model.length
                    lengthWidth: root.lengthWidth
                    meetingId: row.model.meetingId
                    partial: row.model.partial
                    recoverable: row.model.recoverable
                    selected: row.model.meetingId === root.selectedId || row.index === list.currentIndex
                    speakerCount: row.model.speakerCount
                    speakers: row.model.speakers
                    speakersWidth: root.speakersWidth
                    stateWidth: root.stateWidth
                    summary: root.searching && row.model.snippet.length > 0 ? row.model.snippet : row.model.summary
                    title: row.model.title
                    when: row.model.when
                    whenWidth: root.whenWidth
                    width: list.width
                    onActivated: {
                        list.currentIndex = row.index;
                        root.activated(row.model.meetingId);
                    }
                }

                Accessible.role: Accessible.List
                Accessible.name: qsTr("Meetings list")
            }
        }
    }

    StateView {
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        y: viewport.y + noticeText.y + noticeText.height + Theme.space7
        keyHint: root.searching ? "" : "n"
        reason: {
            if (root.searching)
                return root.meetings && root.meetings.error.length > 0 ? root.meetings.error : qsTr("Search covers the title, the transcript, the speakers, the notes and the analysis.");
            return qsTr("Start one from the rail or with n. Audio files can be imported too.");
        }
        title: root.searching ? qsTr("No match for \"%1\".").arg(root.query) : qsTr("No meetings recorded.")
        visible: root.count === 0 && root.loaded
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meetings column")
}
