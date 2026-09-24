// Dettivo style — Menu (R3). Shares Popup.qml's overlay dimming and
// entrance/exit motion, with its own list-shaped background/content.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Menu {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)
    width: Math.min(implicitWidth, parent && parent.Window.window ? parent.Window.window.width - leftMargin - rightMargin : implicitWidth)
    height: Math.min(implicitHeight, parent && parent.Window.window ? parent.Window.window.height - topMargin - bottomMargin : implicitHeight)

    margins: 0
    overlap: 1
    padding: Theme.spacingXs

    delegate: MenuItem {}

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

    contentItem: ListView {
        implicitWidth: {
            let widest = 0;
            for (let i = 0; i < control.count; ++i) {
                const item = control.itemAt(i);
                if (item)
                    widest = Math.max(widest, item.implicitWidth);
            }
            return widest;
        }
        implicitHeight: contentHeight
        model: control.contentModel
        interactive: Window.window ? contentHeight + control.topPadding + control.bottomPadding > control.height : false
        clip: true
        currentIndex: control.currentIndex

        // Attached-property host is QQuickScrollBar, so it takes our
        // ScrollBar style (not a ScrollIndicator, which this style doesn't
        // define — the contract only lists ScrollBar).
        T.ScrollBar.vertical: ScrollBar {}
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 6
        implicitHeight: Theme.controlHeight
        radius: Theme.radius
        color: Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface)
        border.width: StyleHelpers.borderWidth("Normal")
        border.color: Theme.roleBorder

        // T.Menu is not an Item, so Accessible lives on its (Item) background.
        Accessible.role: Accessible.PopupMenu
        Accessible.name: qsTr("Menu")
    }

    T.Overlay.modal: Rectangle {
        color: StyleHelpers.withAlpha(Theme.colorBackground, 0.5)
    }

    T.Overlay.modeless: Rectangle {
        color: StyleHelpers.withAlpha(Theme.colorBackground, 0.12)
    }
}
