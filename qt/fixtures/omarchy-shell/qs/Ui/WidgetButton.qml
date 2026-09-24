import QtQuick
import qs.Commons

// The shell's WidgetButton: the label slot with the active underline.
Item {
    id: root

    property var bar: null
    property string text: ""
    property bool hasVisualContent: text !== ""
    visible: hasVisualContent
    property string fontFamily: Style.font.family
    property real fontSize: Style.font.body
    property color foreground: Color.foreground
    property color activeColor: Color.urgent
    property bool active: false
    property real horizontalMargin: 8.5
    property real fixedWidth: -1
    property real fixedHeight: -1
    property bool dimmed: false
    property bool useActiveColor: true
    property bool labelVisible: true
    property string tooltipText: ""

    signal pressed(int button)

    implicitWidth: fixedWidth > 0 ? fixedWidth : Math.max(12, label.implicitWidth + horizontalMargin * 2)
    implicitHeight: fixedHeight > 0 ? fixedHeight : Style.bar.sizeHorizontal
    opacity: dimmed ? 0.45 : 1

    Text {
        id: label
        visible: root.labelVisible
        anchors.centerIn: parent
        text: root.text
        color: root.active && root.useActiveColor ? root.activeColor : root.foreground
        font.family: root.fontFamily
        font.pixelSize: root.fontSize
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 2
        visible: root.active
        color: root.bar ? root.bar.urgent : Color.urgent
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onClicked: mouse => root.pressed(mouse.button)
    }
}
