import QtQuick
import Dettivo

// One row of a history or meetings list: a leading caption (a time), the
// text, and a trailing slot for a chip or a duration. Hover is the shell's
// hover fill, selection is an 18% accent fill with a 2 px accent rail on
// the leading edge, and rows are separated by a hairline.
Rectangle {
    id: root

    property string leading: ""
    property string text: ""
    property string trailing: ""
    property bool selected: false
    property bool separator: true
    property real horizontalPadding: Theme.rowPaddingX
    property alias trailingItem: trailingSlot.data

    signal activated

    readonly property bool hovered: hoverHandler.hovered

    implicitHeight: Theme.rowHeight
    implicitWidth: Theme.controlHeight * 8
    activeFocusOnTab: true
    border.width: root.activeFocus ? Theme.stateFocusBorderWidth : 0
    border.color: Qt.alpha(Theme.stateFocusColor, Theme.stateFocusBorderAlpha)
    radius: Theme.radius
    color: root.selected ? Theme.roleSelectedFill : (root.hovered ? Theme.roleHoverFill : "transparent")

    Behavior on color {
        enabled: !Motion.reducedMotion
        ColorAnimation {
            duration: Motion.duration(Motion.durationEnter)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingEnter
        }
    }

    Rectangle {
        id: rail
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: Theme.railWidth
        color: Theme.roleSelected
        visible: root.selected
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Theme.hairlineWidth
        color: Theme.roleHairline
        visible: root.separator
    }

    Row {
        anchors.fill: parent
        anchors.leftMargin: root.horizontalPadding
        anchors.rightMargin: root.horizontalPadding
        spacing: Theme.rowPaddingX

        Text {
            id: leadingText
            anchors.verticalCenter: parent.verticalCenter
            visible: root.leading.length > 0
            width: visible ? Theme.controlHeight + Theme.space5 : 0
            text: root.leading
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.features: Theme.typeTabularNumerals
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(0, parent.width - leadingText.width - trailingSlot.width - trailingText.width - parent.spacing * (2 + (trailingSlot.visible ? 1 : 0)))
            text: root.text
            textFormat: Text.PlainText
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            elide: Text.ElideRight
        }

        Item {
            id: trailingSlot
            anchors.verticalCenter: parent.verticalCenter
            visible: children.length > 0
            width: visible ? childrenRect.width : 0
            height: childrenRect.height
        }

        Text {
            id: trailingText
            anchors.verticalCenter: parent.verticalCenter
            visible: root.trailing.length > 0
            width: visible ? implicitWidth : 0
            text: root.trailing
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.features: Theme.typeTabularNumerals
        }
    }

    HoverHandler {
        id: hoverHandler
    }

    TapHandler {
        onTapped: {
            root.forceActiveFocus(Qt.MouseFocusReason);
            root.activated();
        }
    }

    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    Accessible.role: Accessible.ListItem
    Accessible.name: root.text
    Accessible.selectable: true
    Accessible.selected: root.selected
    Accessible.onPressAction: root.activated()
}
