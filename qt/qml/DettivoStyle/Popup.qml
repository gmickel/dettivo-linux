// Dettivo style — Popup (R3). Base popup chrome reused by Menu/ComboBox
// dropdowns and any host-authored popup that doesn't need a bespoke shell.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Popup {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)

    padding: Theme.panelPadding

    contentItem: Item {
        implicitWidth: childrenRect.width
        implicitHeight: childrenRect.height
    }

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0.0
            to: 1.0
            duration: Motion.duration(Motion.durationEnter)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingEnter
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

    background: Rectangle {
        radius: Theme.radius
        color: Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface)
        border.width: StyleHelpers.borderWidth("Normal")
        border.color: Theme.roleBorder

        // T.Popup is not an Item, so the Accessible attached property lives
        // on its (Item) background rather than on `control` itself.
        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Popup")
    }

    T.Overlay.modal: Rectangle {
        color: StyleHelpers.withAlpha(Theme.colorBackground, 0.5)
    }

    T.Overlay.modeless: Rectangle {
        color: StyleHelpers.withAlpha(Theme.colorBackground, 0.12)
    }
}
