pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Level bars 3 px wide with a 2 px gap, each a scene-graph rectangle, fed
// by audio.level events. Idle bars sit at the floor; a live meter sets
// `levels` per event and the bars follow linearly at the level frame rate.
Item {
    id: root

    // Values in [0, 1], one per bar, newest last.
    property var levels: []
    property int barCount: 24
    property color barColor: Theme.roleAccent
    property real floorLevel: 0.12
    property string label: qsTr("Audio level")

    implicitWidth: barCount * (Theme.waveformBarWidth + Theme.waveformBarGap) - Theme.waveformBarGap
    implicitHeight: Theme.controlHeight - Theme.space3

    function levelAt(index) {
        const values = root.levels || [];
        const offset = values.length - root.barCount;
        const value = values[index + offset];
        return value === undefined ? 0 : Math.max(0, Math.min(1, value));
    }

    Row {
        anchors.fill: parent
        spacing: Theme.waveformBarGap

        Repeater {
            model: root.barCount

            delegate: Item {
                id: slot
                required property int index
                width: Theme.waveformBarWidth
                height: root.height

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    radius: Theme.radius
                    color: root.barColor
                    height: Math.max(Theme.space1, parent.height * Math.max(root.floorLevel, root.levelAt(slot.index)))

                    Behavior on height {
                        enabled: !Motion.reducedMotion
                        NumberAnimation {
                            duration: Motion.duration(Motion.levelFrameMs)
                            easing.type: Motion.easingLinear
                        }
                    }
                }
            }
        }
    }

    Accessible.role: Accessible.Graphic
    Accessible.name: root.label
}
