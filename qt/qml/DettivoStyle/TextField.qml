// Dettivo style — TextField (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.TextField {
    id: control

    implicitWidth: implicitBackgroundWidth + leftInset + rightInset || Math.max(contentWidth, placeholder.implicitWidth) + leftPadding + rightPadding
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, contentHeight + topPadding + bottomPadding, Theme.controlHeight)

    padding: Theme.space2
    verticalAlignment: TextInput.AlignVCenter

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    color: Theme.roleText
    selectionColor: StyleHelpers.selectionFillColor()
    selectedTextColor: Theme.roleText
    placeholderTextColor: Theme.roleMutedText

    readonly property string borderState: StyleHelpers.pickBorderState(false, control.activeFocus, control.hovered)

    Accessible.role: Accessible.EditableText
    Accessible.name: control.text.length > 0 ? control.text : control.placeholderText
    Accessible.editable: control.enabled && !control.readOnly
    Accessible.readOnly: !control.enabled || control.readOnly

    // Plain Text rather than the Templates PlaceholderText type: that type
    // is only reliably resolvable through the private *.impl modules the
    // built-in styles use, which this style intentionally does not depend on.
    Text {
        id: placeholder
        x: control.leftPadding
        y: control.topPadding
        width: control.width - (control.leftPadding + control.rightPadding)
        height: control.height - (control.topPadding + control.bottomPadding)

        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: control.verticalAlignment
        visible: !control.length && !control.preeditText && control.text.length === 0
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 6
        implicitHeight: Theme.controlHeight
        radius: Theme.radius

        color: Qt.tint(Theme.roleSurface, StyleHelpers.fillColor(control.borderState))
        border.width: StyleHelpers.borderWidth(control.borderState)
        border.color: StyleHelpers.borderColor(control.borderState)
        opacity: control.enabled ? 1.0 : 0.5
    }
}
