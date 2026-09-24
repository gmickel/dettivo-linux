import QtQuick
import Dettivo

// The analysis rail of the meeting detail (meeting-detail.png): the
// heading with the model and the time, the summary, the decisions and the
// action items, and at its foot the notes, export and audio facts. A
// meeting without an analysis says why and offers to run it; a running
// one shows its stage.
Item {
    id: root

    property var detail: null
    property var actions: null
    property string meetingId: ""

    readonly property string status: root.detail ? root.detail.analysisStatus : "none"
    readonly property bool ready: root.status === "ready"

    FocusFlickable {
        id: scroll
        anchors.top: parent.top
        anchors.bottom: facts.top
        anchors.bottomMargin: Theme.space4
        anchors.left: parent.left
        anchors.right: parent.right
        contentWidth: width
        contentHeight: column.implicitHeight + Theme.space4 * 2

        Column {
            id: column
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            spacing: Theme.space5

            Item {
                height: heading.implicitHeight
                width: parent.width

                Text {
                    id: heading
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeHeadingSize
                    font.weight: Theme.typeEmphasisWeight
                    text: qsTr("Analysis")
                    Accessible.role: Accessible.Heading
                    Accessible.name: qsTr("Analysis rail")
                }

                SectionLabel {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    rightPadding: 0
                    text: root.detail ? root.detail.analysisMeta : ""
                }
            }

            AnalysisBlock {
                label: qsTr("Summary")
                paragraph: root.detail ? root.detail.summary : ""
                visible: root.ready
                width: parent.width
            }

            AnalysisBlock {
                items: root.detail ? root.detail.decisions : []
                label: qsTr("Decisions")
                visible: root.ready && root.detail.decisions.length > 0
                width: parent.width
            }

            AnalysisBlock {
                items: root.detail ? root.detail.actionItems : []
                label: qsTr("Action items")
                visible: root.ready && root.detail.actionItems.length > 0
                width: parent.width
            }

            StateView {
                action: root.status === "running" || root.status === "queued" ? "" : qsTr("Analyse now")
                actionEnabled: root.actions !== null && !root.actions.busy
                reason: {
                    if (root.status === "running")
                        return root.detail && root.detail.stage.length > 0 ? qsTr("The language model is reading the transcript (%1).").arg(root.detail.stage) : qsTr("The language model is reading the transcript.");
                    if (root.status === "queued")
                        return qsTr("It starts once the speaker pass is done; the transcript is usable meanwhile.");
                    if (root.status === "failed")
                        return root.detail && root.detail.analysisError.length > 0 ? root.detail.analysisError : qsTr("The last run failed; the notes are untouched.");
                    return qsTr("Summary, decisions and action items run on the language model over the finalised transcript. Notes stay yours.");
                }
                title: root.status === "running" ? qsTr("Analysing.") : (root.status === "queued" ? qsTr("Analysis queued.") : (root.status === "failed" ? qsTr("Analysis failed.") : qsTr("No analysis yet.")))
                urgent: root.status === "failed"
                visible: !root.ready
                width: parent.width
                onActionTriggered: {
                    if (root.actions)
                        root.actions.analyze(root.meetingId, false);
                }
            }
        }
    }

    Column {
        id: facts
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        spacing: 0

        RailRow {
            name: qsTr("notes")
            value: root.detail ? root.detail.notesMeta : ""
        }

        RailRow {
            name: qsTr("exports")
            value: root.detail ? root.detail.exportFacts : ""
        }

        RailRow {
            name: qsTr("audio")
            value: root.detail ? root.detail.audioFacts : ""
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Analysis")
}
