pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// A panel of facts (agents.png): the tracked label and one hairline row
// per fact, the label on the left and the value right-aligned; a row may
// carry a control instead of a value.
Column {
    id: root

    property string title: ""
    // Each row is { label, value }; a row with `control` names a key the
    // panel's switch edits.
    property var facts: []
    property var settings: null

    spacing: 0

    SectionLabel {
        bottomPadding: Theme.space2
        leftPadding: 0
        text: root.title
        topPadding: 0
    }

    Rectangle {
        color: Theme.roleHairline
        height: Theme.hairlineWidth
        width: parent.width
    }

    Repeater {
        model: root.facts

        delegate: Item {
            id: row
            required property var modelData
            readonly property string controlKey: row.modelData.control ? String(row.modelData.control) : ""
            readonly property int revision: root.settings ? root.settings.revision : 0
            readonly property bool checked: root.readChecked(row.controlKey, row.revision)
            readonly property string valueText: row.controlKey.length > 0 ? (row.checked ? qsTr("on") : qsTr("off")) : String(row.modelData.value)

            height: Theme.settingsFactRowHeight
            width: root.width

            Text {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: String(row.modelData.label)
                Accessible.role: Accessible.StaticText
                Accessible.name: String(row.modelData.label)
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.space3

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleMutedText
                    elide: Text.ElideMiddle
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    horizontalAlignment: Text.AlignRight
                    text: row.valueText
                    width: Math.min(implicitWidth, root.width * 0.7)
                    Accessible.role: Accessible.StaticText
                    Accessible.name: String(row.modelData.label) + ": " + row.valueText
                }

                SettingSwitch {
                    anchors.verticalCenter: parent.verticalCenter
                    checked: row.checked
                    name: row.controlKey
                    visible: row.controlKey.length > 0
                    width: Theme.controlHeight * 1.6
                    onToggled: on => {
                        if (root.settings)
                            root.settings.set(row.controlKey, on);
                    }
                }
            }

            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                color: Theme.roleHairline
                height: Theme.hairlineWidth
            }

            Accessible.role: Accessible.Row
            Accessible.name: String(row.modelData.label)
        }
    }

    function readChecked(key, revision) {
        return key.length > 0 && root.settings && revision >= 0 ? root.settings.value(key) === true : false;
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: root.title
}
