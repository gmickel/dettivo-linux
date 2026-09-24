import QtQuick
import QtQuick.Layouts
import Dettivo

// The motion tokens and the two level components that use them.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Motion · one easing set, reduced motion honoured")
    }
    Rectangle {
        Layout.fillWidth: true
        border.color: Theme.roleHairline
        border.width: Theme.hairlineWidth
        color: "transparent"
        implicitHeight: column.implicitHeight + Theme.space6 * 2

        ColumnLayout {
            id: column
            anchors.fill: parent
            anchors.margins: Theme.space6
            spacing: Theme.space3

            KeyValueRow {
                label: qsTr("enter")
                value: qsTr("%1 ms").arg(Motion.durationEnter)
            }
            KeyValueRow {
                label: qsTr("exit")
                value: qsTr("%1 ms").arg(Motion.durationExit)
            }
            KeyValueRow {
                label: qsTr("reveal")
                value: qsTr("%1 ms").arg(Motion.durationReveal)
            }
            KeyValueRow {
                label: qsTr("pill")
                value: qsTr("spring · %1 ms").arg(Motion.durationPill)
            }
            KeyValueRow {
                label: qsTr("level")
                value: qsTr("linear · per frame")
            }
            KeyValueRow {
                label: qsTr("shimmer")
                value: qsTr("linear · %1 ms loop").arg(Motion.durationShimmer)
            }
            KeyValueRow {
                label: qsTr("reduced motion")
                value: Motion.reducedMotion ? qsTr("on") : qsTr("off")
            }
            RowLayout {
                spacing: Theme.space6

                Waveform {
                    barCount: 11
                    levels: [0.2, 0.5, 0.9, 0.6, 1.0, 0.4, 0.7, 0.3, 0.8, 0.5, 0.2]
                }
                LevelMeter {
                    Layout.fillWidth: true
                    level: 0.6
                    peak: 0.85
                }
            }
        }
    }
}
