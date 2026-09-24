pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The facts of a dictation as a two-column key-value grid (history.png):
// the app, the mode, the engine and the language on the left; the
// stop-to-insert time, the insertion backend, the take and the id on the
// right, because those are the numbers the product promises.
Grid {
    id: root

    // `{label, value}` maps, row-major over the two columns.
    property var facts: []

    columnSpacing: Theme.space6
    columns: 2
    rowSpacing: Theme.space3

    Repeater {
        model: root.facts

        delegate: Item {
            id: cell
            required property var modelData
            height: value.implicitHeight
            width: (root.width - root.columnSpacing) / 2

            Text {
                id: label
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: cell.modelData.label
                width: Theme.space8 * 2 + Theme.space6
            }

            Text {
                id: value
                anchors.left: label.right
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                elide: Text.ElideRight
                font.family: Theme.fontFamily
                font.features: Theme.typeTabularNumerals
                font.pixelSize: Theme.typeBodySize
                text: cell.modelData.value
            }

            Accessible.role: Accessible.Row
            Accessible.name: cell.modelData.label + ": " + cell.modelData.value
        }
    }

    Accessible.role: Accessible.Table
    Accessible.name: qsTr("Facts")
}
