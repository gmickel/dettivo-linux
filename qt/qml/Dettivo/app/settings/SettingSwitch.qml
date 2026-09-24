import QtQuick
import QtQuick.Controls
import Dettivo

// A boolean key: the styled switch, right-aligned, named by the key so a
// drive flips it by name. The key's value drives it; a click writes.
Item {
    id: root

    property bool checked: false
    property string name: ""

    signal toggled(bool on)

    implicitHeight: Theme.controlHeight
    implicitWidth: control.implicitWidth

    Switch {
        id: control
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        checked: root.checked
        onClicked: root.toggled(control.checked)
        Accessible.name: root.name
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("%1 switch").arg(root.name)
}
