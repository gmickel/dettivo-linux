pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Five level bars (meeting-live.png, meetings-list.png): the bars up to
// the current level in the accent, the peak in a faded accent, the rest
// as hairlines; the middle bar tallest so the glyph reads as a meter at
// any size.
Item {
    id: root

    property real level: 0
    property real peak: 0
    property string label: qsTr("Level")

    readonly property int bars: 5

    implicitHeight: Theme.space3 + Theme.space2
    implicitWidth: row.implicitWidth

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.waveformBarGap

        Repeater {
            model: root.bars

            delegate: Rectangle {
                id: bar
                required property int index
                readonly property real threshold: (bar.index + 0.5) / root.bars
                anchors.verticalCenter: parent.verticalCenter
                color: root.level >= bar.threshold ? Theme.roleAccent : (root.peak >= bar.threshold ? Qt.alpha(Theme.roleAccent, 0.45) : Theme.roleHairline)
                height: Theme.space2 + (bar.index === 2 ? Theme.space3 : (bar.index % 2 === 0 ? Theme.space1 : Theme.space2))
                radius: Theme.radius
                width: Theme.waveformBarWidth

                Behavior on color {
                    enabled: !Motion.reducedMotion
                    ColorAnimation {
                        duration: Motion.duration(Motion.levelFrameMs * 3)
                    }
                }
            }
        }
    }

    Accessible.role: Accessible.ProgressBar
    Accessible.name: root.label
}
