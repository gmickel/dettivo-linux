pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The sidebar of every window (home.png): the mark, Workspace and System
// routes with the selected fill, and the three footer rows every window
// shares: the active speech engine, the language model, the socket mode.
Rectangle {
    id: root

    property var router: null
    property var status: null
    property var engines: null

    readonly property string route: root.router ? root.router.route : "home"
    readonly property string sub: root.router ? root.router.sub : ""

    color: Theme.roleRaisedSurface
    implicitWidth: Theme.sidebarWidth

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.roleHairline
        width: Theme.hairlineWidth
    }

    Column {
        id: top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: Theme.space4

        Row {
            height: Theme.rowHeight
            leftPadding: Theme.space4
            spacing: Theme.space2

            Icon {
                accessibleName: ""
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleAccent
                size: Theme.iconSize
                source: "sixbar"
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("dettivo")
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Dettivo")
            }
        }

        SectionLabel {
            leftPadding: Theme.space4
            text: qsTr("Workspace")
            topPadding: Theme.space3
        }

        SidebarItem {
            active: root.route === "home"
            iconName: "home"
            text: qsTr("Home")
            onActivated: root.open("home")
        }

        SidebarItem {
            active: root.route === "history"
            iconName: "history"
            text: qsTr("History")
            onActivated: root.open("history")
        }

        SidebarItem {
            active: root.route === "meetings"
            iconName: "meeting"
            text: qsTr("Meetings")
            onActivated: root.open("meetings")
        }

        SectionLabel {
            leftPadding: Theme.space4
            text: qsTr("System")
            topPadding: Theme.space5
        }

        SidebarItem {
            active: root.route === "settings" && root.sub !== "agents"
            iconName: "settings"
            text: qsTr("Settings")
            onActivated: root.open("settings.general")
        }

        SidebarItem {
            active: root.route === "settings" && root.sub === "agents"
            iconName: "agents"
            text: qsTr("Agents")
            onActivated: root.open("agents")
        }
    }

    function open(name) {
        if (root.router)
            root.router.open(name);
    }

    Column {
        id: footer
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space4
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4 + Theme.hairlineWidth
        spacing: Theme.space2

        SidebarFooterRow {
            accent: value === "warm"
            label: root.engines && root.engines.speechName.length > 0 ? root.engines.speechName : qsTr("speech")
            role: qsTr("Speech engine")
            value: root.engines && root.engines.speechState.length > 0 ? root.engines.speechState : qsTr("idle")
        }

        SidebarFooterRow {
            accent: value === "warm"
            label: root.engines && root.engines.languageModelName.length > 0 ? root.engines.languageModelName : qsTr("language model")
            role: qsTr("Language model")
            value: root.engines && root.engines.languageModelState.length > 0 ? root.engines.languageModelState : qsTr("idle")
        }

        SidebarFooterRow {
            accent: false
            label: qsTr("socket")
            role: qsTr("Socket mode")
            value: root.status && root.status.socketMode.length > 0 ? root.status.socketMode : (root.status && root.status.daemonState === "away" ? qsTr("away") : qsTr("peer"))
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Sidebar")
}
