pragma ComponentBehavior: Bound
import QtQuick
import DettivoStyle as Style
import Dettivo

Rectangle {
    id: root

    property var model: []
    property int currentIndex: 0
    property string label: ""

    signal selected(int index)

    // The delegate item for a segment, for tests and keyboard handling.
    function segmentAt(index) {
        return repeater.itemAt(index);
    }

    implicitHeight: Theme.controlHeight
    implicitWidth: root.model.reduce((width, label) => Math.max(width, metrics.advanceWidth(String(label))), 0) * root.model.length + (Theme.rowPaddingX * 2 + Theme.space1) * root.model.length + Theme.space1
    radius: Theme.radius
    color: Theme.roleHoverFill
    border.width: Theme.stateNormalBorderWidth
    border.color: Theme.roleBorder

    FontMetrics {
        id: metrics
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
    }

    Row {
        id: row
        anchors.fill: parent
        anchors.margins: Theme.space1
        spacing: Theme.space1

        Repeater {
            id: repeater
            model: root.model

            delegate: Style.SegmentButton {
                id: segment
                required property int index
                required property var modelData
                active: index === root.currentIndex
                width: (row.width - row.spacing * Math.max(0, root.model.length - 1)) / Math.max(1, root.model.length)
                height: row.height
                text: String(segment.modelData)
                onClicked: root.selected(segment.index)
                Keys.onRightPressed: root.segmentAt((segment.index + 1) % root.model.length).forceActiveFocus(Qt.TabFocusReason)
                Keys.onLeftPressed: root.segmentAt((segment.index + root.model.length - 1) % root.model.length).forceActiveFocus(Qt.TabFocusReason)
            }
        }
    }

    Accessible.role: Accessible.PageTabList
    Accessible.name: root.label
}
