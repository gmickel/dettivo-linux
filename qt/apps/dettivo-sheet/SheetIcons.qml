pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Dettivo

// The sixteen icons on the 16 px grid, recoloured by the text role.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Icons · 16 px grid, 1.5 px stroke, square caps")
    }
    Rectangle {
        Layout.fillWidth: true
        border.color: Theme.roleHairline
        border.width: Theme.hairlineWidth
        color: "transparent"
        implicitHeight: flow.implicitHeight + Theme.space6 * 2

        Flow {
            id: flow
            anchors.fill: parent
            anchors.margins: Theme.space6
            spacing: Theme.space3

            Repeater {
                model: ["mic", "wave", "history", "meeting", "settings", "agents", "insert", "copy", "rerun", "export", "delete", "search", "close", "app", "home", "engine"]

                delegate: Column {
                    id: glyph
                    required property string modelData
                    spacing: Theme.space3
                    width: Theme.space8 + Theme.space5

                    Icon {
                        accessibleName: glyph.modelData
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: Theme.roleText
                        size: Theme.iconSizeLarge
                        source: glyph.modelData
                    }
                    SectionLabel {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: glyph.modelData
                    }
                }
            }
        }
    }
}
