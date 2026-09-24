import QtQuick
import QtQuick.Controls
import Dettivo

// The head of the live meeting (meeting-live.png): a recording square in
// the accent, the title, the two source meters with their device and app
// names, the elapsed time as the largest type on screen, Pause (disabled:
// the contract has no pause verb, the reason is its description) and
// Stop. The moment Stop is accepted the square turns muted, the elapsed
// display freezes on the recorded length and the meters give way to the
// finalisation: `Recording stopped`, the stage with its chunk count and
// a progress bar (a sweep until the daemon has counted the chunks). A
// refusal from the daemon reads in the urgent colour in the same place.
Item {
    id: root

    property var live: null

    readonly property bool finishing: root.live ? root.live.finishing : false
    readonly property string error: root.live ? root.live.error : ""
    readonly property string stateLine: {
        if (!root.live)
            return "";
        if (root.live.state === "transcribing" || root.live.state === "stopped")
            return root.live.chunksTotal > 0 ? qsTr("Transcribing · %1 of %2 chunks").arg(root.live.chunksDone).arg(root.live.chunksTotal) : qsTr("Transcribing · building the transcript from the takes");
        if (root.finishing)
            return qsTr("Stopping · closing the takes");
        return "";
    }
    readonly property real finishProgress: root.live && root.live.chunksTotal > 0 ? root.live.chunksDone / root.live.chunksTotal : -1

    signal stopRequested

    implicitHeight: Theme.space8 * 2 - Theme.space1

    Rectangle {
        id: square
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY
        color: root.finishing ? Theme.roleMutedText : Theme.roleAccent
        height: Theme.space3
        radius: Theme.radius
        width: Theme.space3

        SequentialAnimation on opacity {
            loops: Animation.Infinite
            running: !Motion.reducedMotion && !root.finishing && root.visible
            NumberAnimation {
                duration: Motion.duration(Motion.durationShimmer)
                to: 0.35
            }
            NumberAnimation {
                duration: Motion.duration(Motion.durationShimmer)
                to: 1
            }
        }

        Accessible.role: Accessible.Graphic
        Accessible.name: root.finishing ? qsTr("Stopping") : qsTr("Recording")
    }

    Text {
        id: title
        anchors.left: square.right
        anchors.leftMargin: Theme.space4
        anchors.right: elapsed.left
        anchors.rightMargin: Theme.space4
        anchors.verticalCenter: square.verticalCenter
        color: Theme.roleText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
        font.pixelSize: Theme.typeTitleSize
        font.weight: Theme.typeEmphasisWeight
        text: root.live && root.live.title.length > 0 ? root.live.title : qsTr("Meeting")
        textFormat: Text.PlainText
        Accessible.role: Accessible.Heading
        Accessible.name: text
    }

    Row {
        id: meters
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.top: title.bottom
        anchors.topMargin: Theme.space4
        spacing: Theme.space5
        visible: !root.finishing

        SourceMeter {
            device: root.live ? root.live.micDevice : ""
            label: qsTr("Mic")
            level: root.live ? root.live.micLevel : 0
            peak: root.live ? root.live.micPeak : 0
        }

        SourceMeter {
            device: root.live ? root.live.systemDevice : ""
            label: qsTr("System")
            level: root.live ? root.live.systemLevel : 0
            peak: root.live ? root.live.systemPeak : 0
            visible: root.live ? root.live.systemAudio : true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleUrgent
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.error
            visible: text.length > 0
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Live notice")
            Accessible.description: text
        }
    }

    Column {
        id: finish
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: elapsed.left
        anchors.rightMargin: Theme.space4
        anchors.top: title.bottom
        anchors.topMargin: Theme.space4
        spacing: Theme.space2
        visible: root.finishing

        Row {
            spacing: Theme.space3

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("Recording stopped")
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Recording stopped")
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.features: Theme.typeTabularNumerals
                font.pixelSize: Theme.typeCaptionSize
                text: root.stateLine
                visible: text.length > 0
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Finalisation stage")
                Accessible.description: text
            }
        }

        ProgressBar {
            id: finishBar
            from: 0
            indeterminate: root.finishProgress < 0
            to: 1
            value: Math.max(0, root.finishProgress)
            width: Theme.space8 * 5
            Accessible.name: qsTr("Finalisation progress")
        }
    }

    Text {
        id: elapsed
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.space3
        color: root.finishing ? Theme.roleMutedText : Theme.roleText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeDisplaySize
        font.weight: Theme.typeEmphasisWeight
        text: root.live ? root.live.elapsedText : "00:00:00"
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Elapsed")
        Accessible.description: text
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: elapsed.bottom
        anchors.topMargin: Theme.space1
        spacing: Theme.space2

        Button {
            enabled: false
            text: qsTr("Pause")
            Accessible.description: qsTr("The meeting contract has no pause; stop the meeting and start another instead.")
        }

        Button {
            enabled: !root.finishing
            highlighted: true
            icon.name: "stop"
            text: qsTr("Stop")
            onClicked: root.stopRequested()
            Accessible.description: root.finishing ? qsTr("The stop was accepted; the transcript is being built.") : ""
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Live header")
}
