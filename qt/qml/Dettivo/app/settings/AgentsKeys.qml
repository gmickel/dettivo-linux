import QtQuick
import Dettivo

// The `[ipc]` and `[mcp]` rows of the Agents route, on the settings
// pattern under the host table.
Column {
    id: root

    property var settings: null

    spacing: 0

    SettingsGroup {
        title: qsTr("Socket keys")

        SettingRow {
            choices: ["peer", "peer_token"]
            hint: qsTr("peer_token adds the shared token to every request")
            key: "ipc.auth_mode"
            kind: "choice"
            label: qsTr("Auth mode")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("empty means $XDG_RUNTIME_DIR/dettivo/dettivo.sock")
            key: "ipc.socket"
            label: qsTr("Socket path")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("mode 0600; empty means $XDG_CONFIG_HOME/dettivo/ipc.token")
            key: "ipc.token_file"
            label: qsTr("Token file")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("bytes a request line may hold")
            key: "ipc.max_line_bytes"
            label: qsTr("Line limit")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("MCP keys")

        SettingRow {
            hint: qsTr("dettivo mcp config adds the DETTIVO_IPC_TOKEN placeholder")
            key: "mcp.hardened"
            kind: "switch"
            label: qsTr("Hardened hosts")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("bytes one MCP message may hold")
            key: "mcp.max_message_bytes"
            label: qsTr("Message limit")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("REST shim keys")

        SettingRow {
            hint: qsTr("the daemon hosts the loopback shim when it starts")
            key: "rest.enabled"
            kind: "switch"
            label: qsTr("Host at start")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("a loopback address only")
            key: "rest.bind"
            label: qsTr("Bind address")
            placeholder: qsTr("127.0.0.1")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("0 takes an ephemeral port")
            key: "rest.port"
            label: qsTr("Port")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("bytes one request body may hold")
            key: "rest.max_body_bytes"
            label: qsTr("Body limit")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds one request may take")
            key: "rest.request_timeout_ms"
            label: qsTr("Request timeout")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Agent keys")
}
