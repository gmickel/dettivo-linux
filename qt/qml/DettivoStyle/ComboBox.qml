// Dettivo style — ComboBox (R3). Reuses this directory's Popup and
// ItemDelegate styles for the dropdown rather than redrawing them.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Templates as T
import QtQuick.Window
import Dettivo

T.ComboBox {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    leftPadding: padding
    rightPadding: padding + (indicator ? indicator.width + spacing : 0)
    spacing: Theme.spacingXs

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    readonly property string borderState: StyleHelpers.pickBorderState(false, control.visualFocus, control.hovered)

    Accessible.role: Accessible.ComboBox
    Accessible.name: control.displayText

    delegate: ItemDelegate {
        required property var model
        required property int index

        width: ListView.view.width
        text: model[control.textRole]
        highlighted: control.highlightedIndex === index
        hoverEnabled: control.hoverEnabled
    }

    indicator: Icon {
        x: control.mirrored ? control.padding : control.width - width - control.padding
        y: control.topPadding + (control.availableHeight - height) / 2
        accessibleName: ""
        color: Theme.roleMutedText
        opacity: control.enabled ? 1.0 : 0.5
        size: Theme.iconSize
        source: "chevron-down"
    }

    contentItem: T.TextField {
        implicitHeight: contentHeight + topPadding + bottomPadding
        leftPadding: 0
        rightPadding: 0

        text: control.editable ? control.editText : control.displayText
        enabled: control.editable
        autoScroll: control.editable
        readOnly: control.down
        inputMethodHints: control.inputMethodHints
        validator: control.validator
        selectByMouse: control.selectTextByMouse

        font: control.font
        color: Theme.roleText
        selectionColor: StyleHelpers.selectionFillColor()
        selectedTextColor: Theme.roleText
        verticalAlignment: Text.AlignVCenter

        background: null
        // The box is the control: it carries the role and the name, and
        // its display text is not a second, nameless field.
        Accessible.ignored: true
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 5
        implicitHeight: Theme.controlHeight
        radius: Theme.radius

        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.down ? "Pressed" : "Normal"))
        border.width: StyleHelpers.borderWidth(control.borderState)
        border.color: StyleHelpers.borderColor(control.borderState)
        opacity: control.enabled ? 1.0 : 0.5
    }

    popup: Popup {
        y: control.height + Theme.spacingXs
        width: control.width
        height: Math.min(contentItem.implicitHeight, control.Window.height - topMargin - bottomMargin) // qmllint disable missing-property

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.delegateModel
            currentIndex: control.highlightedIndex
            highlightMoveDuration: Motion.duration(Motion.durationReveal)

            T.ScrollBar.vertical: ScrollBar {}
        }
    }
}
