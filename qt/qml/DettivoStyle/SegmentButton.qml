import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Button {
    id: control
    property bool active: false
    padding: 0
    hoverEnabled: true
    implicitWidth: label.implicitWidth + Theme.rowPaddingX * 2
    Keys.onReturnPressed: control.click()
    Keys.onEnterPressed: control.click()
    contentItem: Text {
        id: label
        text: control.text
        color: control.active ? Theme.roleText : Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.down ? StyleHelpers.fillColor("Pressed") : (control.active ? Theme.roleSelectedFill : (control.hovered ? Theme.roleHoverFill : "transparent"))
        border.width: control.active ? Theme.hairlineWidth : 0
        border.color: Qt.alpha(Theme.roleSelected, Theme.stateSelectedBorderAlpha)
        Behavior on color {
            enabled: !Motion.reducedMotion
            ColorAnimation {
                duration: Motion.duration(Motion.durationEnter)
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easingEnter
            }
        }
    }
    FocusRing {
        visible: control.visualFocus
    }
    Accessible.role: Accessible.PageTab
    Accessible.name: text
    Accessible.selected: active
    Accessible.onPressAction: control.click()
}
