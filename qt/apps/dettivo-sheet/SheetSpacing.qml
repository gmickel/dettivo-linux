pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Dettivo

// The spacing scale and the shape tokens with the values they resolve to.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Spacing and shape · shell tokens, scale with font")
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
            spacing: Theme.space4

            Row {
                spacing: Theme.space4

                Repeater {
                    model: [Theme.space1, Theme.space2, Theme.space3, Theme.space4, Theme.space5, Theme.space6, Theme.space7, Theme.space8]

                    delegate: Column {
                        id: step
                        required property int modelData
                        anchors.bottom: parent.bottom
                        spacing: Theme.space2

                        Rectangle {
                            color: Theme.roleAccent
                            height: step.modelData
                            width: step.modelData
                        }
                        SectionLabel {
                            text: String(step.modelData)
                        }
                    }
                }
            }
            KeyValueRow {
                label: qsTr("control height")
                value: String(Theme.controlHeight)
            }
            KeyValueRow {
                label: qsTr("row padding x")
                value: String(Theme.rowPaddingX)
            }
            KeyValueRow {
                label: qsTr("control gap")
                value: String(Theme.controlGap)
            }
            KeyValueRow {
                label: qsTr("corner radius")
                value: String(Theme.radius)
            }
        }
    }
}
