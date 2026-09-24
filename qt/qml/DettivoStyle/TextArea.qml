// Dettivo style — TextArea: a multi-line field on the theme's monospace
// type with the selection fill; the frame around it is the caller's (the
// first-run Try it panel draws its own), so the background stays clear.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.TextArea {
    id: control

    implicitWidth: Math.max(contentWidth + leftPadding + rightPadding, implicitBackgroundWidth + leftInset + rightInset, placeholder.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(contentHeight + topPadding + bottomPadding, implicitBackgroundHeight + topInset + bottomInset, Theme.controlHeight)

    padding: Theme.space2

    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBaseSize

    color: Theme.roleText
    selectionColor: StyleHelpers.selectionFillColor()
    selectedTextColor: Theme.roleText
    placeholderTextColor: Theme.roleMutedText
    wrapMode: TextEdit.Wrap

    Accessible.role: Accessible.EditableText
    Accessible.name: control.text.length > 0 ? control.text : control.placeholderText
    Accessible.editable: true
    Accessible.readOnly: control.readOnly

    Text {
        id: placeholder
        x: control.leftPadding
        y: control.topPadding
        width: control.width - (control.leftPadding + control.rightPadding)
        height: control.height - (control.topPadding + control.bottomPadding)

        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: Text.AlignTop
        visible: !control.length && !control.preeditText && control.text.length === 0
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 6
        implicitHeight: Theme.controlHeight * 3
        color: "transparent"
    }
}
