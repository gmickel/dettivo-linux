pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Hotkeys (settings-hotkeys.png): the four chords as editable rows, the
// snippet the daemon renders from them with the rewrite and the copy
// action beside the toggle panel (the daemon backend, media pause, the
// sounds, the evdev devices), all over `[hotkeys]`.
SettingsPage {
    id: root

    property var setup: null
    property var status: null

    readonly property string compositor: root.setup && root.setup.compositor.length > 0 ? root.setup.compositor.charAt(0).toUpperCase() + root.setup.compositor.slice(1) : qsTr("The compositor")
    readonly property bool omarchy: Theme.source === "omarchy"

    section: "hotkeys"
    subtitle: qsTr("%1 owns the keys. Dettivo writes a snippet your config includes and never registers global hooks.").arg(root.compositor)

    trailing: Chip {
        accent: true
        text: root.omarchy ? qsTr("%1 · Omarchy").arg(root.compositor) : root.compositor
    }

    Item {
        height: Theme.space5 + Theme.space1
        width: parent.width

        SectionLabel {
            anchors.left: parent.left
            leftPadding: 0
            text: qsTr("Action")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.firstRunActionColumn
            leftPadding: 0
            text: qsTr("Binding")
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    HotkeyRow {
        action: qsTr("Hold to talk")
        hint: qsTr("bind on press, bindr on release")
        key: "hotkeys.hold"
        settings: root.settings
        width: parent.width
    }

    HotkeyRow {
        action: qsTr("Toggle dictation")
        key: "hotkeys.toggle"
        settings: root.settings
        width: parent.width
    }

    HotkeyRow {
        action: qsTr("Cancel")
        hint: qsTr("while listening")
        key: "hotkeys.cancel"
        settings: root.settings
        width: parent.width
    }

    HotkeyRow {
        action: qsTr("Re-insert last")
        key: "hotkeys.reinsert"
        settings: root.settings
        width: parent.width
    }

    // The artboard leaves a binding row's worth of air before the snippet.
    Item {
        height: Theme.space5
        width: parent.width
    }

    Column {
        spacing: Theme.space5
        width: parent.width

        Column {
            id: snippetColumn
            spacing: Theme.space3
            width: parent.width

            SectionLabel {
                bottomPadding: 0
                leftPadding: 0
                text: root.setup && root.setup.supported ? qsTr("Snippet · %1").arg(root.setup.snippetPath) : qsTr("Portal bindings")
                topPadding: 0
            }

            Rectangle {
                visible: root.advanced
                border.color: Theme.roleBorder
                border.width: Theme.stateNormalBorderWidth
                color: "transparent"
                height: snippetText.implicitHeight + Theme.space4 * 2
                radius: Theme.radius
                width: parent.width

                Text {
                    id: snippetText
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.space4
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.space4
                    anchors.top: parent.top
                    anchors.topMargin: Theme.space4
                    color: Theme.roleText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeCaptionSize
                    lineHeight: 1.35
                    text: root.setup && root.setup.supported ? root.setup.snippetText.trim() : (root.setup ? root.setup.statusLine : "")
                    wrapMode: Text.WrapAnywhere
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text.length > 0 ? text : qsTr("Snippet")
                }

                Accessible.role: Accessible.Pane
                Accessible.name: qsTr("Bindings file")
            }

            Row {
                spacing: Theme.space3

                Button {
                    enabled: root.setup ? root.setup.supported : false
                    text: qsTr("Apply shortcuts")
                    onClicked: {
                        if (root.setup)
                            root.setup.rewrite();
                    }
                }

                Button {
                    enabled: root.setup ? root.setup.includeLine.length > 0 : false
                    text: qsTr("Copy include line")
                    onClicked: {
                        if (root.setup)
                            root.setup.copyIncludeLine();
                    }
                }
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.setup && root.setup.outcome.length > 0 ? root.setup.outcome : (root.setup ? root.setup.statusLine : "")
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Snippet state")
            }
        }

        Column {
            id: panel
            spacing: 0
            width: parent.width

            SettingRow {
                choices: ["auto", "portal", "evdev", "none"]
                hint: qsTr("GNOME and KDE")
                key: "hotkeys.backend"
                kind: "choice"
                label: qsTr("Daemon backend")
                segmentLimit: 0
                settings: root.settings
                showKey: false
                width: parent.width
            }

            SettingRow {
                hint: qsTr("MPRIS players")
                key: "hotkeys.pause_media"
                showKey: false
                kind: "switch"
                label: qsTr("Pause media while listening")
                settings: root.settings
                width: parent.width
            }

            SettingRow {
                hint: qsTr("start, stop, error; never in meetings")
                key: "hotkeys.sounds"
                showKey: false
                kind: "switch"
                label: qsTr("Feedback sounds")
                settings: root.settings
                width: parent.width
            }

            SettingRow {
                hint: qsTr("evdev input devices")
                key: "hotkeys.evdev_devices"
                showKey: false
                label: qsTr("evdev devices")
                placeholder: qsTr("every keyboard")
                separator: false
                settings: root.settings
                width: parent.width
            }
        }
    }
}
