import QtQuick
import QtQuick.Templates as T
import Dettivo

// One route in the sidebar: an icon and its name, the shell's selected
// fill when the route is open, the hover fill otherwise.
T.Button {
    id: root

    property string iconName: ""
    property bool active: false

    signal activated

    padding: 0
    onClicked: root.activated()
    Keys.onReturnPressed: root.click()
    Keys.onEnterPressed: root.click()

    height: Theme.rowHeight - Theme.space1
    width: parent ? parent.width : implicitWidth
    background: Rectangle {
        color: root.active ? Theme.roleSelectedFill : (root.hovered ? Theme.roleHoverFill : "transparent")
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
            visible: root.active
            width: Theme.railWidth
        }
    }

    contentItem: Row {
        anchors.fill: parent
        anchors.leftMargin: Theme.space4
        spacing: Theme.space3

        Icon {
            accessibleName: ""
            anchors.verticalCenter: parent.verticalCenter
            color: root.active ? Theme.roleText : Theme.roleMutedText
            size: Theme.iconSize
            source: root.iconName
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: root.active ? Theme.roleText : Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.text
        }
    }

    FocusRing {
        visible: root.visualFocus
    }

    Accessible.role: Accessible.Button
    Accessible.name: root.text
    Accessible.selected: root.active
}
