import QtQuick
import QtQuick.Controls
import Dettivo

// The foot of the import dialog (import-and-disclosure.png): the command
// line that does the same on the left, Cancel and the primary Import on
// the right; Import is held until a readable meeting recording is named.
Item {
    id: root

    property bool ready: false

    signal cancelled
    signal accepted

    implicitHeight: importButton.implicitHeight

    Text {
        anchors.left: parent.left
        anchors.right: cancelButton.left
        anchors.rightMargin: Theme.space4
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: qsTr("Also: dettivo transcript import --file <path> --target-kind meeting")
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Button {
        id: cancelButton
        anchors.right: importButton.left
        anchors.rightMargin: Theme.space2
        anchors.verticalCenter: parent.verticalCenter
        text: qsTr("Cancel")
        onClicked: root.cancelled()
    }

    Button {
        id: importButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        enabled: root.ready
        highlighted: true
        text: qsTr("Import")
        onClicked: root.accepted()
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Import actions")
}
