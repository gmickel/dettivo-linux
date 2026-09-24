pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Settings (ADR 0033): the section column beside the sidebar and the
// section the router names, each an editor over config.toml through the
// settings model. Advanced mode reveals technical details; the file is
// one click away.
Item {
    id: root

    property var router: null
    property var config: null
    property var settings: null
    property var modelsTable: null
    property var hotkeysSetup: null
    property var agents: null
    property var doctor: null
    property var status: null

    readonly property var sections: root.router ? root.router.settingsSections : ["general", "vocabulary", "hotkeys", "models", "polish", "insertion", "meetings", "agents", "diagnostics"]
    readonly property string sub: root.router && root.router.sub.length > 0 ? root.router.sub : "general"
    readonly property string sectionTitle: root.sub.charAt(0).toUpperCase() + root.sub.slice(1)
    readonly property string routeTitle: qsTr("Settings / %1").arg(root.sectionTitle)
    // Agents is its own page (agents.png): the column names its blocks
    // and a click scrolls to one.
    readonly property bool agentsPage: root.sub === "agents"
    readonly property var agentAnchors: ["overview", "hosts", "rest", "cli"]
    readonly property var agentLabels: ({
            "overview": qsTr("Overview"),
            "hosts": qsTr("MCP hosts"),
            "rest": qsTr("REST shim"),
            "cli": qsTr("CLI")
        })
    property string agentAnchor: "overview"
    readonly property var pageItem: page.item
    // The router's argument names a key to scroll into view.
    readonly property string keyArg: root.router ? root.router.arg : ""

    onKeyArgChanged: root.showKey()

    function showKey() {
        if (root.pageItem && root.pageItem.scrollToKey && root.keyArg.length > 0)
            Qt.callLater(() => root.pageItem.scrollToKey(root.keyArg));
    }

    SettingsNav {
        id: nav
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.top: parent.top
        current: root.agentsPage && SettingsUi.advanced ? root.agentAnchor : root.sub
        labels: root.agentsPage && SettingsUi.advanced ? root.agentLabels : ({})
        sections: root.agentsPage && SettingsUi.advanced ? root.agentAnchors : root.sections
        settings: root.settings
        title: root.agentsPage ? qsTr("Agents") : qsTr("Settings")
        onOpened: section => {
            if (root.agentsPage && SettingsUi.advanced) {
                root.agentAnchor = section;
                if (root.pageItem && root.pageItem.jumpTo)
                    root.pageItem.jumpTo(section);
            } else if (root.router && section !== root.sub) {
                root.router.open("settings." + section);
            }
        }
    }

    Loader {
        id: page
        anchors.bottom: parent.bottom
        anchors.left: nav.right
        anchors.right: parent.right
        anchors.top: parent.top
        sourceComponent: {
            switch (root.sub) {
            case "vocabulary":
                return vocabularyPage;
            case "hotkeys":
                return hotkeysPage;
            case "models":
                return modelsPage;
            case "polish":
                return polishPage;
            case "insertion":
                return insertionPage;
            case "meetings":
                return meetingsPage;
            case "agents":
                return agentsPage;
            case "diagnostics":
                return diagnosticsPage;
            default:
                return generalPage;
            }
        }
        onLoaded: root.showKey()
    }

    Component {
        id: generalPage
        GeneralSection {
            settings: root.settings
            title: root.routeTitle
        }
    }

    Component {
        id: vocabularyPage
        VocabularySection {
            settings: root.settings
            title: root.routeTitle
        }
    }

    Component {
        id: hotkeysPage
        HotkeysSection {
            settings: root.settings
            setup: root.hotkeysSetup
            status: root.status
            title: root.routeTitle
        }
    }

    Component {
        id: modelsPage
        ModelsSection {
            settings: root.settings
            table: root.modelsTable
            title: root.routeTitle
        }
    }

    Component {
        id: polishPage
        PolishSection {
            settings: root.settings
            title: root.routeTitle
        }
    }

    Component {
        id: insertionPage
        InsertionSection {
            settings: root.settings
            title: root.routeTitle
        }
    }

    Component {
        id: meetingsPage
        MeetingsSection {
            table: root.modelsTable
            settings: root.settings
            title: root.routeTitle
        }
    }

    Component {
        id: agentsPage
        AgentsSection {
            agents: root.agents
            settings: root.settings
            status: root.status
            title: root.routeTitle
        }
    }

    Component {
        id: diagnosticsPage
        DiagnosticsSection {
            doctor: root.doctor
            settings: root.settings
            title: root.routeTitle
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.routeTitle
}
