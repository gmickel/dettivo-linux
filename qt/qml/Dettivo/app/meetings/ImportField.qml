import QtQuick
import Dettivo

// One labelled row of the import dialog (import-and-disclosure.png): the
// muted label in a fixed column on the left, the control filling the rest.
Item {
    id: root

    property string label: ""
    default property alias control: slot.data

    readonly property int labelWidth: Theme.space8 * 2 + Theme.space6

    implicitHeight: Math.max(slot.childrenRect.height, labelText.implicitHeight)

    Text {
        id: labelText
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.label
        width: root.labelWidth
    }

    Item {
        id: slot
        anchors.left: labelText.right
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: childrenRect.height
    }

    Accessible.role: Accessible.Row
    Accessible.name: root.label.length > 0 ? root.label : qsTr("File facts row")
}
