pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// The section column of Settings (settings-models.png): the title, the
// eight sections with the open one marked by the accent rail, and the
// file at the foot with the way to open it, so config.toml is never a
// hidden second system.
Rectangle {
    id: root

    property var sections: []
    property string current: "general"
    property var settings: null
    // The column's heading and the labels of its entries; a section with
    // no label reads as its capitalised name.
    property string title: qsTr("Settings")
    property var labels: ({})

    signal opened(string section)

    readonly property string configPath: root.settings && root.settings.configPath.length > 0 ? root.settings.configPath : "~/.config/dettivo/config.toml"

    color: "transparent"
    implicitWidth: Theme.settingsNavWidth

    function labelFor(section) {
        return root.labels[section] ? root.labels[section] : section.charAt(0).toUpperCase() + section.slice(1);
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.roleHairline
        width: Theme.hairlineWidth
    }

    Text {
        id: heading
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5
        anchors.top: parent.top
        anchors.topMargin: Theme.space5 + Theme.space1
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeHeadingSize
        font.weight: Theme.typeEmphasisWeight
        text: root.title
        Accessible.role: Accessible.Heading
        Accessible.name: qsTr("Settings sections")
    }

    Button {
        id: mode
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5
        anchors.top: heading.bottom
        anchors.topMargin: Theme.space3
        text: qsTr("Advanced settings")
        checkable: true
        checked: SettingsUi.advanced
        Accessible.name: qsTr("Show advanced settings")
        onClicked: SettingsUi.advanced = !SettingsUi.advanced
    }

    Column {
        id: list
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.rightMargin: Theme.hairlineWidth
        anchors.top: mode.bottom
        anchors.topMargin: Theme.space4 + Theme.space1

        Repeater {
            model: root.sections

            delegate: Rectangle {
                id: item
                required property string modelData
                readonly property bool active: item.modelData === root.current
                readonly property bool hovered: hover.hovered

                color: item.active ? Theme.roleSelectedFill : (item.hovered ? Theme.roleHoverFill : "transparent")
                height: Theme.controlHeight + Theme.space1
                radius: Theme.radius
                width: list.width

                Rectangle {
                    anchors.bottom: parent.bottom
                    anchors.left: parent.left
                    anchors.top: parent.top
                    color: Theme.roleSelected
                    visible: item.active
                    width: Theme.railWidth
                }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.space5
                    anchors.verticalCenter: parent.verticalCenter
                    color: item.active ? Theme.roleText : Theme.roleMutedText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    text: root.labelFor(item.modelData)
                }

                HoverHandler {
                    id: hover
                }

                TapHandler {
                    onTapped: root.opened(item.modelData)
                }

                Accessible.role: Accessible.PageTab
                Accessible.name: root.labelFor(item.modelData)
                Accessible.selected: item.active
                Accessible.onPressAction: root.opened(item.modelData)
            }
        }

        Accessible.role: Accessible.PageTabList
        Accessible.name: qsTr("Sections")
    }

    Column {
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space5
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        spacing: Theme.space2

        Text {
            color: Theme.roleFaintText
            elide: Text.ElideMiddle
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.configPath
            width: parent.width
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Configuration file")
        }

        Text {
            color: Theme.roleAccent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("open in editor")

            TapHandler {
                onTapped: {
                    if (root.settings)
                        root.settings.openConfig();
                }
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    if (root.settings)
                        root.settings.openConfig();
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Open config.toml")
            Accessible.onPressAction: {
                if (root.settings)
                    root.settings.openConfig();
            }
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Settings navigation")
}
