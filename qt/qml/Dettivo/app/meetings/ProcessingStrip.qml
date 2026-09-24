pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// What still happens to a meeting after the stop (ADR 0061): a bordered
// strip under the header with the sentence that says what is ready and
// what runs next, the three stages (transcript, speakers, analysis) each
// with its dot in the state's colour, its label and its detail, and a
// progress bar under the running stage (a sweep until it has a count).
// The strip stays with the meeting: in the accent while any stage is
// pending, queued or running, in the urgent colour once a stage failed,
// in the highlight colour when a wanted pass had no model to run on
// (`incomplete`), and muted once every stage settled well, so the
// outcome and each stage's facts (segments, speakers and the segments
// left unassigned, the model and the time) stay readable; the
// transcript underneath is usable all the while.
Item {
    id: root

    property var detail: null

    readonly property var stages: root.detail && root.detail.stages !== undefined ? root.detail.stages : []
    readonly property string processingState: root.detail && root.detail.processingState !== undefined ? root.detail.processingState : "done"
    readonly property string line: root.detail && root.detail.processingLine !== undefined ? root.detail.processingLine : ""
    readonly property var running: root.stages.find(s => s.state === "running") || null
    readonly property bool failed: root.processingState === "failed"
    readonly property bool settled: root.processingState !== "running"
    readonly property color tone: {
        if (root.failed)
            return Theme.roleUrgent;
        if (root.processingState === "incomplete")
            return Theme.roleHighlight;
        return root.settled ? Theme.roleMutedText : Theme.roleAccent;
    }

    function stateWord(state) {
        switch (state) {
        case "done":
            return qsTr("done");
        case "running":
            return qsTr("running");
        case "queued":
            return qsTr("queued");
        case "pending":
            return qsTr("pending");
        case "failed":
            return qsTr("failed");
        case "unavailable":
            return qsTr("skipped");
        default:
            return qsTr("not run");
        }
    }

    function dotStatus(state) {
        if (state === "failed")
            return StatusDot.Urgent;
        if (state === "unavailable")
            return StatusDot.Selected;
        if (state === "skipped" || state === "pending")
            return StatusDot.Neutral;
        return root.failed ? StatusDot.Neutral : StatusDot.Accent;
    }

    implicitHeight: visible ? box.implicitHeight : 0
    visible: root.stages.length > 0

    Rectangle {
        id: box
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        border.color: Qt.alpha(root.tone, root.settled && !root.failed ? 0.3 : 0.5)
        border.width: Theme.hairlineWidth
        color: Qt.alpha(root.tone, root.settled && !root.failed ? 0.03 : 0.06)
        implicitHeight: column.implicitHeight + Theme.space4 * 2
        radius: Theme.radius

        Column {
            id: column
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            spacing: Theme.space3

            Text {
                color: Theme.roleText
                elide: Text.ElideRight
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: root.line
                width: parent.width
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Processing")
                Accessible.description: text
            }

            Flow {
                id: stages
                spacing: Theme.space6
                width: parent.width

                Repeater {
                    model: root.stages

                    delegate: Row {
                        id: cell
                        required property var modelData
                        spacing: Theme.space2

                        StatusDot {
                            id: dot
                            active: cell.modelData.state === "running"
                            anchors.verticalCenter: parent.verticalCenter
                            label: cell.modelData.label + ": " + root.stateWord(cell.modelData.state)
                            status: root.dotStatus(cell.modelData.state)
                        }

                        Text {
                            id: label
                            anchors.verticalCenter: parent.verticalCenter
                            color: cell.modelData.state === "failed" ? Theme.roleUrgent : (cell.modelData.state === "unavailable" ? Theme.roleHighlight : (cell.modelData.state === "skipped" || cell.modelData.state === "pending" ? Theme.roleFaintText : Theme.roleText))
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.typeBodySize
                            font.weight: cell.modelData.state === "running" ? Theme.typeEmphasisWeight : Font.Normal
                            text: cell.modelData.label
                            Accessible.role: Accessible.StaticText
                            Accessible.name: cell.modelData.label
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            color: cell.modelData.state === "failed" ? Theme.roleUrgent : (cell.modelData.state === "unavailable" ? Theme.roleHighlight : Theme.roleMutedText)
                            elide: Text.ElideRight
                            font.family: Theme.fontFamily
                            font.features: Theme.typeTabularNumerals
                            font.pixelSize: Theme.typeCaptionSize
                            text: root.stateWord(cell.modelData.state) + (cell.modelData.detail.length > 0 ? " · " + cell.modelData.detail : "")
                            // The room the row has: the strip's width less the dot,
                            // the label and the gaps, so a long detail (3 speakers,
                            // 154 segments unassigned) takes a line of the flow to
                            // itself instead of eliding at a fixed cap.
                            width: Math.min(implicitWidth, stages.width - dot.width - label.width - cell.spacing * 2)
                            Accessible.role: Accessible.StaticText
                            Accessible.name: cell.modelData.label + " state"
                            Accessible.description: text
                        }
                    }
                }
            }

            ProgressBar {
                from: 0
                indeterminate: !root.running || root.running.progress < 0
                to: 1
                value: root.running && root.running.progress >= 0 ? root.running.progress : 0
                visible: root.running !== null
                width: parent.width
                Accessible.name: qsTr("Processing progress")
            }
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Processing strip")
}
