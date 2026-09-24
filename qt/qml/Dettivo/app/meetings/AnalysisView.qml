import QtQuick
import QtQuick.Controls
import Dettivo

// The Analysis tab: the same summary, decisions and action items as the
// rail at reading width, with Analyse again to regenerate (the old
// analysis stays until the new one parses) and Run speakers again for
// the diarization pass.
Item {
    id: root

    property var detail: null
    property var actions: null
    property string meetingId: ""

    readonly property string status: root.detail ? root.detail.analysisStatus : "none"

    Flickable {
        anchors.fill: parent
        clip: true
        contentHeight: column.implicitHeight + Theme.space5

        Column {
            id: column
            anchors.left: parent.left
            anchors.right: parent.right
            spacing: Theme.space5

            Row {
                spacing: Theme.space2

                Button {
                    enabled: root.actions !== null && !root.actions.busy && root.status !== "running" && root.status !== "queued"
                    icon.name: "rerun"
                    text: root.status === "ready" ? qsTr("Analyse again") : qsTr("Analyse now")
                    onClicked: {
                        if (root.actions)
                            root.actions.analyze(root.meetingId, root.status === "ready");
                    }
                }

                Button {
                    enabled: root.actions !== null && !root.actions.busy && (root.detail ? root.detail.diarizationStatus !== "running" && root.detail.diarizationStatus !== "queued" : true)
                    text: qsTr("Run speakers again")
                    onClicked: {
                        if (root.actions)
                            root.actions.diarize(root.meetingId, 0);
                    }
                }
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.detail && root.detail.progress >= 0 ? qsTr("%1 · %2 %").arg(root.detail.stage).arg(Math.round(root.detail.progress * 100)) : ""
                visible: text.length > 0
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Analysis progress")
            }

            AnalysisBlock {
                label: qsTr("Summary")
                paragraph: root.detail && root.status === "ready" ? root.detail.summary : qsTr("No analysis yet.")
                width: parent.width
            }

            AnalysisBlock {
                items: root.detail && root.status === "ready" ? root.detail.decisions : []
                label: qsTr("Decisions")
                visible: items.length > 0
                width: parent.width
            }

            AnalysisBlock {
                items: root.detail && root.status === "ready" ? root.detail.actionItems : []
                label: qsTr("Action items")
                visible: items.length > 0
                width: parent.width
            }
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Analysis tab")
}
