import QtQuick
import QtQuick.Controls
import Dettivo

// A deliberately plain window with one text field. QA specs drive text
// insertion into this field and read it back over AT-SPI.
Window {
    id: root

    color: Theme.roleSurface
    height: 160
    title: qsTr("Dettivo insert target")
    visible: true
    width: 480

    TextField {
        id: target

        anchors.centerIn: parent
        focus: true
        objectName: "insertTarget"
        placeholderText: qsTr("Insert text here")
        width: parent.width - Theme.space3 * 4
        Accessible.name: qsTr("Insert target")
    }
}
