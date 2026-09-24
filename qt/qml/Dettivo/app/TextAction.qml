import QtQuick
import Dettivo

// A text action (the sheet's `delete` in the urgent colour at the foot
// of a detail): caption-sized text that acts on a tap, takes focus from
// Tab, fires on Return or Space and draws the shell's focus ring.
Text {
    id: root

    property string accessibleName: root.text
    property bool urgent: true

    signal triggered

    activeFocusOnTab: true
    color: root.urgent ? Theme.roleUrgent : Theme.roleMutedText
    font.family: Theme.fontFamily
    font.pixelSize: Theme.typeCaptionSize

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.triggered();
            event.accepted = true;
        }
    }

    TapHandler {
        onTapped: root.triggered()
    }

    FocusRing {}

    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Accessible.onPressAction: root.triggered()
}
