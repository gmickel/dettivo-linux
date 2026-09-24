// Dettivo style — ToolTip (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.ToolTip {
    id: control

    x: parent ? (parent.width - implicitWidth) / 2 : 0
    y: -implicitHeight - Theme.spacingXs

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)

    margins: Theme.spacingSm
    padding: Theme.space2

    closePolicy: T.Popup.CloseOnEscape | T.Popup.CloseOnPressOutsideParent | T.Popup.CloseOnReleaseOutsideParent

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0.0
            to: 1.0
            duration: Motion.duration(Motion.durationReveal)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingReveal
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1.0
            to: 0.0
            duration: Motion.duration(Motion.durationExit)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingExit
        }
    }

    contentItem: Text {
        // T.ToolTip is a Popup, not an Item, so the accessible role rides
        // on its content item.
        Accessible.role: Accessible.ToolTip
        Accessible.name: control.text
        text: control.text
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        wrapMode: Text.Wrap
        color: Theme.roleText
    }

    background: Rectangle {
        radius: Theme.radius
        color: Theme.roleRaisedSurface
        border.width: StyleHelpers.borderWidth("Normal")
        border.color: Theme.roleBorder
    }
}
