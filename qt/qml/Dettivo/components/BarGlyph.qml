pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The 16 px six-bar mark for the Omarchy bar (osd.png, the bar glyph
// strip): five states carried by the bar colour and the bar heights, a
// meeting timer beside the mark, and `dimmed` for the hint state when the
// daemon or the module is missing. Every bar is a scene-graph rectangle;
// the shell draws the active underline, this item draws the mark only.
Item {
    id: root

    // The item's `state`: idle, listening, transcribing, meeting or error;
    // anything else is idle.
    // 0..1 from audio.level while listening; the bars follow it when
    // `levelMeter` is on.
    property real level: 0
    property bool levelMeter: true
    // "waveform" (the six bars) or "dot" ([omarchy] glyph).
    property string glyph: "waveform"
    // The meeting timer, "23:41", shown beside the mark while meeting.
    property string elapsed: ""
    // The hint state: the mark at faint text strength.
    property bool dimmed: false
    property bool reducedMotion: false

    readonly property var knownStates: ["idle", "listening", "transcribing", "meeting", "error"]
    readonly property string effectiveState: knownStates.indexOf(root.state) >= 0 ? root.state : "idle"
    readonly property bool listening: root.effectiveState === "listening"
    readonly property bool transcribing: root.effectiveState === "transcribing"
    readonly property bool meeting: root.effectiveState === "meeting"
    readonly property bool error: root.effectiveState === "error"
    readonly property bool live: (root.listening || root.meeting) && root.levelMeter
    readonly property bool reduced: root.reducedMotion || Motion.reducedMotion
    readonly property int size: Theme.iconSize
    readonly property int barWidth: Theme.hairlineWidth * 2
    readonly property int barGap: Theme.hairlineWidth
    readonly property var shape: [0.35, 0.7, 1.0, 0.55, 0.85, 0.4]
    readonly property color markColor: {
        if (root.dimmed)
            return Theme.roleFaintText;
        if (root.error)
            return Theme.roleUrgent;
        if (root.transcribing)
            return Qt.alpha(Theme.roleAccent, 0.45);
        if (root.listening || root.meeting)
            return Theme.roleAccent;
        return Theme.roleMutedText;
    }
    // What the glyph says, for the accessible name.
    readonly property string sentence: {
        if (root.dimmed)
            return qsTr("Dettivo not ready");
        switch (root.effectiveState) {
        case "listening":
            return qsTr("Dettivo listening");
        case "transcribing":
            return qsTr("Dettivo transcribing");
        case "meeting":
            return root.elapsed.length > 0 ? qsTr("Meeting %1").arg(root.elapsed) : qsTr("Meeting recording");
        case "error":
            return qsTr("Dettivo error");
        default:
            return qsTr("Dettivo idle");
        }
    }

    function barHeight(index) {
        const base = root.shape[index] * root.size;
        if (!root.live)
            return Math.max(root.barWidth, base);
        const floor = 0.3;
        const gain = floor + (1 - floor) * Math.max(0, Math.min(1, root.level));
        return Math.max(root.barWidth, base * gain);
    }

    state: "idle"
    states: [
        State {
            name: "idle"
        },
        State {
            name: "listening"
        },
        State {
            name: "transcribing"
        },
        State {
            name: "meeting"
        },
        State {
            name: "error"
        }
    ]

    implicitWidth: row.implicitWidth
    implicitHeight: root.size

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Item {
            id: mark
            width: root.glyph === "dot" ? root.size : bars.width
            height: root.size

            Row {
                id: bars
                visible: root.glyph !== "dot"
                anchors.verticalCenter: parent.verticalCenter
                spacing: root.barGap

                Repeater {
                    model: 6

                    delegate: Rectangle {
                        id: bar
                        required property int index
                        anchors.verticalCenter: parent.verticalCenter
                        width: root.barWidth
                        height: root.barHeight(bar.index)
                        radius: Theme.radius
                        color: root.markColor

                        Behavior on height {
                            enabled: !root.reduced
                            NumberAnimation {
                                duration: Motion.duration(Motion.levelFrameMs)
                                easing.type: Motion.easingLinear
                            }
                        }
                    }
                }
            }

            Rectangle {
                visible: root.glyph === "dot"
                anchors.centerIn: parent
                width: root.size / 2
                height: width
                radius: Theme.radius
                color: root.markColor
            }
        }

        Text {
            id: timer
            visible: root.meeting && root.elapsed.length > 0
            anchors.verticalCenter: parent.verticalCenter
            text: root.elapsed
            color: root.dimmed ? Theme.roleFaintText : Theme.roleAccent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            font.weight: Theme.typeEmphasisWeight
            font.features: Theme.typeTabularNumerals
        }
    }

    Accessible.role: Accessible.Graphic
    Accessible.name: root.sentence
    Accessible.description: root.effectiveState
}
