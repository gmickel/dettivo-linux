import QtQuick
import QtQuick.Layouts
import Dettivo

// A simple "label: value" row, e.g. in an inspector/detail panel.
RowLayout {
    id: root

    property string label: ""
    property string value: ""

    spacing: Theme.space2
    Layout.fillWidth: true

    Text {
        text: root.label
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        Layout.preferredWidth: implicitWidth
    }

    Text {
        text: root.value
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.features: ({
                "tnum": 1
            })
        elide: Text.ElideRight
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignRight
    }

    Accessible.role: Accessible.Row
    Accessible.name: root.label + ": " + root.value
}
