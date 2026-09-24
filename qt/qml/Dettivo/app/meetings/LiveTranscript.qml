pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// The live transcript (meeting-live.png): the tracked line naming the
// engine and the window, `Source-aligned` on the right, then one row per
// segment with the time, the source label in its speaker colour and the
// text at a 1.7 line height; the provisional tail in the light weight
// and the muted colour until a window finalises it. The view keeps its
// end in sight as segments arrive.
Item {
    id: root

    property var live: null

    readonly property var segments: root.live ? root.live.segments : null
    readonly property string engine: root.live && root.live.engineLabel.length > 0 ? root.live.engineLabel : qsTr("selected engine")

    Item {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        height: Theme.controlHeight

        SectionLabel {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            leftPadding: 0
            text: qsTr("Live transcript · %1 · 3 s windows").arg(root.engine)
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 0
            text: qsTr("Source-aligned")
        }
    }

    ListView {
        id: list
        property bool followingTail: true

        function settlePosition() {
            list.forceLayout();
            if (list.moving || scrollBar.pressed)
                return;
            if (list.contentHeight <= list.height)
                list.positionViewAtBeginning();
            else if (list.followingTail)
                list.positionViewAtEnd();
        }
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        clip: true
        model: root.segments
        spacing: Theme.space3
        ScrollBar.vertical: ScrollBar {
            id: scrollBar
            onPressedChanged: list.followingTail = !pressed && list.atYEnd
        }

        delegate: TranscriptRow {
            id: row
            required property var model
            colorIndex: row.model.colorIndex
            gapBefore: row.model.gapBefore
            label: row.model.label
            provisional: row.model.provisional
            text: row.model.text
            time: row.model.time
            width: list.width
        }

        onCountChanged: Qt.callLater(list.settlePosition)
        onContentHeightChanged: Qt.callLater(list.settlePosition)
        onHeightChanged: Qt.callLater(list.settlePosition)
        onWidthChanged: Qt.callLater(list.settlePosition)
        onMovementStarted: list.followingTail = false
        onMovementEnded: list.followingTail = list.atYEnd

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Live transcript")
    }

    StateView {
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space5
        reason: qsTr("The first window reaches the engine after a few seconds of speech; a quiet room adds nothing.")
        title: qsTr("Listening.")
        visible: !root.segments || root.segments.count === 0
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Transcript")
}
