import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Button {
    id: control
    padding: 0
    implicitWidth: Theme.iconSize
    implicitHeight: Theme.iconSize
    contentItem: Icon {
        accessibleName: ""
        color: Theme.roleMutedText
        size: Theme.iconSize
        source: "close"
    }
    background: FocusRing {
        visible: control.visualFocus
    }
    Accessible.role: Accessible.Button
    Accessible.name: text
}
