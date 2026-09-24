pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The live meeting (meeting-live.png): the header with the recording
// square, the title, both meters, the elapsed display, Pause and Stop;
// the transcript with segments arriving provisional then final in the
// source colours; the labelling-rule footer; the notes editor and the
// analysis card in the rail with the disclosure state at its foot. Stop
// is acknowledged at once: the header turns to the finalisation and the
// footer says the meeting opens when the transcript is ready. Without a
// meeting the designed state says how to start one.
Item {
    id: root

    property var router: null
    property var live: null
    property string notice: ""

    readonly property bool active: root.live ? root.live.active : false
    readonly property bool typing: notes.typing
    readonly property int segmentCount: root.live && root.live.segments ? root.live.segments.finalCount : 0
    readonly property int gapCount: root.live && root.live.segments ? root.live.segments.gapCount : 0

    function confirmStop() {
        if (root.active)
            stopConfirm.open();
    }

    function dismiss() {
        if (stopConfirm.visible) {
            stopConfirm.close();
            return true;
        }
        return false;
    }

    RouteScaffold {
        anchors.fill: parent
        configKeys: qsTr("[speech] meeting_model · [audio] level_interval_ms")
        keyHint: "n"
        stateReason: root.notice.length > 0 ? root.notice : qsTr("Start one with n from Meetings; the sources, the meters and the transcript run here.")
        stateTitle: qsTr("No meeting is being recorded.")
        subtitle: qsTr("Sources, meters, the transcript and your notes while it runs.")
        title: qsTr("Meeting live")
        urgent: root.notice.length > 0
        visible: !root.active
    }

    Item {
        id: main
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: rail.left
        anchors.top: parent.top
        visible: root.active

        LiveHeader {
            id: header
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            live: root.live
            onStopRequested: root.confirmStop()
        }

        LiveTranscript {
            id: transcript
            anchors.bottom: footer.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: header.bottom
            live: root.live
        }

        MeetingsFooter {
            id: footer
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            hairline: true
            hints: root.live && root.live.finishing ? qsTr("Recording stopped · the transcript is built from the takes; the meeting opens when it is ready.") : qsTr("Speakers are labelled by source now. Names arrive after the diarization pass.")
            trailing: root.gapCount === 1 ? qsTr("%1 segments · 1 gap").arg(root.segmentCount) : qsTr("%1 segments · %2 gaps").arg(root.segmentCount).arg(root.gapCount)
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.right: rail.left
        anchors.top: parent.top
        color: Theme.roleHairline
        visible: root.active
        width: Theme.hairlineWidth
    }

    Item {
        id: rail
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.top: parent.top
        visible: root.active
        width: Theme.rightRailWidth + Theme.space8

        NotesEditor {
            id: notes
            anchors.bottom: analysis.top
            anchors.bottomMargin: Theme.space4
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            saveState: root.live ? root.live.notesState : ""
            text: root.live ? root.live.notes : ""
            onEdited: markdown => {
                if (root.live)
                    root.live.setNotes(markdown);
            }
        }

        AnalysisCard {
            id: analysis
            anchors.bottom: disclosure.top
            anchors.bottomMargin: Theme.space4
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
        }

        Item {
            id: disclosure
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            height: footer.height

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                color: Theme.roleHairline
                height: Theme.hairlineWidth
            }

            Text {
                anchors.left: parent.left
                anchors.right: copyLink.left
                anchors.rightMargin: Theme.space3
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleFaintText
                elide: Text.ElideRight
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.live && root.live.disclosureAt.length > 0 ? qsTr("Disclosure acknowledged · %1").arg(root.live.disclosureAt) : qsTr("Disclosure acknowledged")
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Disclosure state")
            }

            Text {
                id: copyLink
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleAccent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("copy message")

                TapHandler {
                    onTapped: {
                        if (root.live)
                            root.live.copyDisclosure();
                    }
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        if (root.live)
                            root.live.copyDisclosure();
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Copy message")
                Accessible.onPressAction: {
                    if (root.live)
                        root.live.copyDisclosure();
                }
            }
        }
    }

    StopConfirm {
        id: stopConfirm
        anchors.centerIn: parent
        live: root.live
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Live meeting")
}
