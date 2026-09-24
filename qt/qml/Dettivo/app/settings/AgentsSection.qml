pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Agents (agents.png): the socket and its auth, the REST shim's state,
// the MCP hosts with their files and the write action that runs
// `dettivo mcp config`, the entry a host receives, and the `[ipc]` and
// `[mcp]` keys the route writes.
SettingsPage {
    id: root

    property var agents: null
    property var status: null

    readonly property string socketPath: root.readText("ipc.socket", root.revision)
    readonly property string authMode: root.readText("ipc.auth_mode", root.revision)
    readonly property string version: root.status && root.status.version.length > 0 ? root.status.version : "1.0.0"
    readonly property var hosts: root.agents ? root.agents.hosts : []

    function readText(key, revision) {
        return root.settings && revision >= 0 ? root.settings.text(key) : "";
    }

    // The column's entries: the panels, the host table, the REST panel
    // (beside the socket at the top) and the entry a host receives.
    function jumpTo(anchor) {
        switch (anchor) {
        case "hosts":
            root.scrollTo(hostsLabel.y);
            break;
        case "cli":
            root.scrollTo(entryBlock.y);
            break;
        default:
            root.scrollTo(0);
        }
    }

    section: "agents"
    subtitle: qsTr("Connect your assistant to local transcripts and tools. Advanced settings show connection and authentication details.")

    Item {
        visible: root.advanced
        height: Math.max(socketPanel.implicitHeight, restPanel.implicitHeight)
        width: parent.width

        FactPanel {
            id: socketPanel
            anchors.left: parent.left
            facts: [
                {
                    "label": qsTr("path"),
                    "value": root.socketPath
                },
                {
                    "label": qsTr("auth"),
                    "value": root.authMode === "peer_token" ? qsTr("peer_token · same uid plus the token") : qsTr("peer · same uid only")
                },
                {
                    "label": qsTr("hardened token"),
                    "control": "mcp.hardened"
                },
                {
                    "label": qsTr("line limit"),
                    "value": qsTr("%1 bytes").arg(root.readText("ipc.max_line_bytes", root.revision))
                },
                {
                    "label": qsTr("api"),
                    "value": qsTr("v%1 · the macOS contract").arg(root.version)
                }
            ]
            settings: root.settings
            title: qsTr("Socket")
            width: Theme.settingsFactPanelWidth
        }

        FactPanel {
            id: restPanel
            anchors.right: parent.right
            facts: [
                {
                    "label": qsTr("enabled"),
                    "value": root.status ? root.status.restState : qsTr("off")
                },
                {
                    "label": qsTr("bind"),
                    "value": qsTr("loopback only")
                },
                {
                    "label": qsTr("token"),
                    "value": qsTr("required when enabled")
                },
                {
                    "label": qsTr("keys"),
                    "value": qsTr("[rest] enabled · port")
                },
                {
                    "label": qsTr("health check"),
                    "value": qsTr("dettivo status ping")
                }
            ]
            title: qsTr("REST shim")
            width: Theme.settingsFactPanelWidth
        }
    }

    Item {
        height: Theme.space6 + Theme.space1
        width: parent.width
    }

    SectionLabel {
        id: hostsLabel
        bottomPadding: 0
        leftPadding: 0
        text: qsTr("Connect an assistant")
        topPadding: 0
    }

    Item {
        height: Theme.space5 + Theme.space1
        width: parent.width

        SectionLabel {
            leftPadding: 0
            text: qsTr("Host")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8 * 4 + Theme.space1
            leftPadding: 0
            visible: root.advanced
            text: qsTr("Config file")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8 * 11 + Theme.space8 / 2 + Theme.space2
            leftPadding: 0
            text: qsTr("State")
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    Column {
        id: hostRows
        spacing: 0
        width: parent.width

        Repeater {
            model: root.hosts

            delegate: AgentHostRow {
                id: row
                required property var modelData
                busy: root.agents ? root.agents.busy : false
                configured: row.modelData.configured
                entryKey: row.modelData.entryKey
                hostId: row.modelData.id
                name: row.modelData.name
                path: row.modelData.path
                stateText: row.modelData.state
                width: hostRows.width
                onWrite: {
                    if (root.agents)
                        root.agents.writeHost(row.modelData.id);
                }
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("MCP hosts")
    }

    Text {
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.agents ? root.agents.lastError : ""
        topPadding: Theme.space3
        visible: text.length > 0
        width: parent.width
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Host write refused")
    }

    Item {
        height: Theme.space6
        width: parent.width
    }

    Column {
        id: entryBlock
        visible: root.advanced
        spacing: Theme.space3
        width: parent.width

        SectionLabel {
            bottomPadding: 0
            leftPadding: 0
            text: qsTr("What Claude Code gets")
            topPadding: 0
        }

        Rectangle {
            border.color: Theme.roleBorder
            border.width: Theme.stateNormalBorderWidth
            color: "transparent"
            height: entryText.implicitHeight + Theme.space4 * 2
            radius: Theme.radius
            width: Theme.settingsSnippetWidth - Theme.space6

            Text {
                id: entryText
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
                text: root.agents && root.agents.entryText.length > 0 ? root.agents.entryText : qsTr("dettivo mcp config --host claude-code prints it.")
                wrapMode: Text.WrapAnywhere
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Host entry")
            }
        }

        Text {
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.agents ? root.agents.toolsLine : ""
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("MCP tools")
        }
    }

    AgentsKeys {
        settings: root.settings
        width: parent.width
    }
}
