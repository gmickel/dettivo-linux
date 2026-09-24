import QtQuick
import Dettivo

// One row of History (history.png): the time in tabular numerals, one
// line of text with the search matches painted in the 12 % accent fill,
// the meta line under it (app, mode, duration; a Meeting chip and its
// length for a meeting), the hairline between rows, and the progress
// hairline along the bottom while a re-run works on the item.
Rectangle {
    id: root

    property string time: ""
    property string text: ""
    property string meta: ""
    property string trailingChip: ""
    property var matches: []
    property real progress: -1
    property bool selected: false

    signal activated

    readonly property bool hovered: hoverHandler.hovered
    readonly property bool painted: root.matches && root.matches.length > 0
    readonly property bool working: root.progress >= 0

    color: root.selected ? Theme.roleSelectedFill : (root.hovered ? Theme.roleHoverFill : "transparent")
    implicitHeight: Theme.historyRowHeight
    implicitWidth: Theme.historyListWidth
    radius: Theme.radius

    Behavior on color {
        enabled: !Motion.reducedMotion
        ColorAnimation {
            duration: Motion.duration(Motion.durationEnter)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingEnter
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.top: parent.top
        color: Theme.roleSelected
        visible: root.selected
        width: Theme.railWidth
    }

    Text {
        id: timeText
        anchors.left: parent.left
        anchors.leftMargin: Theme.rowPaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.space4
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.time
        width: Theme.controlHeight + Theme.space5
    }

    Text {
        id: body
        anchors.left: timeText.right
        anchors.leftMargin: Theme.space2
        anchors.right: parent.right
        anchors.rightMargin: Theme.rowPaddingX
        anchors.top: timeText.top
        color: Theme.roleText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        maximumLineCount: 1
        text: root.painted ? Html.highlighted(root.text, root.matches, Theme.roleSearchHighlight) : root.text
        textFormat: root.painted ? Text.RichText : Text.PlainText
    }

    Row {
        anchors.left: body.left
        anchors.top: body.bottom
        anchors.topMargin: Theme.space1
        spacing: Theme.space2

        Chip {
            anchors.verticalCenter: parent.verticalCenter
            text: root.trailingChip
            visible: root.trailingChip.length > 0
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeCaptionSize
            text: root.meta
            width: Math.min(implicitWidth, root.width - timeText.width - Theme.rowPaddingX * 3)
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        color: Theme.roleAccent
        height: Theme.progressHairlineWidth
        visible: root.working
        width: parent.width * Math.max(0.04, Math.min(1, root.progress))

        Behavior on width {
            enabled: !Motion.reducedMotion
            NumberAnimation {
                duration: Motion.duration(Motion.durationEnter)
                easing.type: Motion.easingLinear
            }
        }
    }

    HoverHandler {
        id: hoverHandler
    }

    TapHandler {
        onTapped: root.activated()
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.text
    Accessible.description: root.meta
    Accessible.selectable: true
    Accessible.selected: root.selected
    Accessible.onPressAction: root.activated()
}
