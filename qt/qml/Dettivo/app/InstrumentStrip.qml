import QtQuick
import QtQuick.Controls
import Dettivo

// The single bordered instrument strip (home.png): the mode segment bound
// to `dictation.mode`, the input with its rate, the waveform that idles at
// the floor and follows `audio.level` while a take runs, and the two
// primary actions.
Rectangle {
    id: root

    property var status: null
    property var config: null
    property bool meetingActive: false

    readonly property string dictationState: root.status ? root.status.dictationState : "idle"
    readonly property bool recording: root.dictationState === "recording"
    readonly property string mode: root.configuredMode(root.config ? root.config.revision : 0)
    readonly property var modes: ["raw", "polish", "enhanced"]
    readonly property string inputName: root.status && root.status.inputName.length > 0 ? root.status.inputName : qsTr("no input")
    readonly property string inputRate: root.status ? root.status.inputRate : ""

    signal modeSelected(string mode)
    signal meetingRequested

    // `revision` is read so the binding follows every config refresh.
    function configuredMode(revision) {
        if (!root.config || revision < 0)
            return "raw";
        const value = root.config.value("dictation.mode");
        return value ? String(value) : "raw";
    }

    border.color: Theme.roleBorder
    border.width: Theme.stateNormalBorderWidth
    color: "transparent"
    implicitHeight: Theme.instrumentStripHeight
    radius: Theme.radius

    Column {
        id: modeColumn
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5 + Theme.space1
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space3

        SectionLabel {
            leftPadding: 0
            text: qsTr("Mode")
        }

        SegmentedControl {
            id: segment
            currentIndex: Math.max(0, root.modes.indexOf(root.mode))
            label: qsTr("Dictation mode")
            model: [qsTr("Raw"), qsTr("Polish"), qsTr("Enhanced")]
            onSelected: index => {
                const chosen = root.modes[index];
                root.modeSelected(chosen);
                if (root.config)
                    root.config.set("dictation.mode", chosen);
            }
        }
    }

    Column {
        id: inputColumn
        anchors.left: modeColumn.right
        anchors.leftMargin: Theme.space6 + Theme.space1
        anchors.right: actions.left
        anchors.rightMargin: Theme.space6 + Theme.space1
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space3

        Item {
            height: inputLabel.implicitHeight
            width: parent.width

            SectionLabel {
                id: inputLabel
                anchors.left: parent.left
                bottomPadding: 0
                leftPadding: 0
                text: qsTr("Input · %1 · %2").arg(root.inputName).arg(root.inputRate)
                topPadding: 0
            }

            SectionLabel {
                anchors.right: parent.right
                bottomPadding: 0
                leftPadding: 0
                text: root.recording ? qsTr("Live") : (root.dictationState === "idle" ? qsTr("Idle") : root.dictationState)
                topPadding: 0
            }
        }

        Waveform {
            id: wave
            barColor: root.recording ? Theme.roleAccent : Theme.roleFaintText
            barCount: Math.max(1, Math.floor(parent.width / (Theme.waveformBarWidth + Theme.waveformBarGap)))
            height: Theme.space5
            levels: root.status ? root.status.levels : []
            width: parent.width
        }
    }

    Column {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: Theme.space5 + Theme.space1
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Button {
            enabled: root.status ? root.status.daemonConnected : false
            highlighted: true
            icon.name: root.recording ? "stop" : "mic"
            text: root.recording ? qsTr("Stop dictation") : qsTr("Start dictation")
            width: Theme.controlHeight * 5 + Theme.space3
            onClicked: {
                if (root.status)
                    root.status.toggleDictation();
            }
        }

        Button {
            enabled: root.status ? root.status.daemonConnected && (root.meetingActive || root.dictationState === "idle") : false
            icon.name: "meeting"
            text: root.meetingActive ? qsTr("Return to meeting") : qsTr("Start meeting")
            width: Theme.controlHeight * 5 + Theme.space3
            onClicked: root.meetingRequested()
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Instrument strip")
}
