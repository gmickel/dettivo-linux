import QtQuick
import Dettivo

// Insertion: the chain over `[insert]`, the paste keys and the app id
// lists, the clipboard restore and the undo window.
SettingsPage {
    id: root

    section: "insertion"
    subtitle: qsTr("How the text lands. auto runs the chain in order; a pinned backend fails with its reason and never falls back.")

    SettingsGroup {
        title: qsTr("Chain")

        SettingRow {
            choices: ["auto", "virtual_keyboard", "libei", "ydotool", "xdotool", "clipboard_paste", "clipboard"]
            choiceLabels: [qsTr("Automatic"), qsTr("Wayland keyboard"), qsTr("Desktop portal"), qsTr("ydotool"), qsTr("X11 keyboard"), qsTr("Copy and paste"), qsTr("Clipboard only")]
            hint: qsTr("the backend, or auto for the chain")
            key: "insert.backend"
            kind: "choice"
            label: qsTr("Backend")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds between typed keys")
            key: "insert.inter_key_delay_ms"
            label: qsTr("Inter-key delay")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Paste path")

        SettingRow {
            hint: qsTr("app id=keystroke; ctrl+v, ctrl+shift+v or shift+insert")
            key: "insert.paste_keys"
            label: qsTr("Paste keys")
            placeholder: qsTr("foot=ctrl+shift+v, Code=ctrl+v")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("app ids that take ctrl+shift+v, comma separated")
            key: "insert.terminal_app_ids"
            label: qsTr("Terminal app ids")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("Dettivo's own windows; insertion into one fails")
            key: "insert.self_app_ids"
            label: qsTr("Self app ids")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("put the previous clipboard back after a paste")
            key: "insert.restore_clipboard"
            kind: "switch"
            label: qsTr("Restore the clipboard")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds the restore waits for the paste")
            key: "insert.clipboard_restore_delay_ms"
            label: qsTr("Restore delay")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Undo")

        SettingRow {
            hint: qsTr("milliseconds dettivo insert undo may take it back")
            key: "insert.undo_window_ms"
            label: qsTr("Undo window")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
