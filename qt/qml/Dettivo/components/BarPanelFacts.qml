pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The three fact rows of the panel: ENGINE, ENHANCED and INSERT INTO with
// their values right-aligned, a hairline between rows.
Column {
    id: root

    required property var panel

    readonly property var rows: [
        {
            "label": qsTr("Engine"),
            "value": root.panel.engine
        },
        {
            "label": qsTr("Enhanced"),
            "value": root.panel.enhanced
        },
        {
            "label": qsTr("Insert into"),
            "value": root.panel.insertTarget
        }
    ]

    spacing: 0

    Rectangle {
        width: parent.width
        height: Theme.hairlineWidth
        color: Theme.roleHairline
    }

    Repeater {
        model: root.rows

        delegate: Item {
            id: row
            required property var modelData
            required property int index
            readonly property string label: String(row.modelData.label)
            readonly property string value: String(row.modelData.value || "")

            width: root.width
            height: Theme.controlHeight + Theme.space1

            SectionLabel {
                id: rowLabel
                anchors.left: parent.left
                anchors.leftMargin: Theme.rowPaddingX - Theme.spacingXs
                anchors.verticalCenter: parent.verticalCenter
                text: row.label
            }

            Text {
                anchors.right: parent.right
                anchors.rightMargin: Theme.rowPaddingX
                anchors.left: rowLabel.right
                anchors.leftMargin: Theme.space3
                anchors.verticalCenter: parent.verticalCenter
                text: row.value.length > 0 ? row.value : "–"
                color: row.value.length > 0 ? Theme.roleMutedText : Theme.roleFaintText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideLeft
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: Theme.hairlineWidth
                color: Theme.roleHairline
            }

            Accessible.role: Accessible.Row
            Accessible.name: row.label + ": " + row.value
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Facts")
}
