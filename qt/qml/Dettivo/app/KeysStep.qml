pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Keys (first-run-1-keys.png): the compositor detected, the four bindings
// with the commands they run, the exact snippet the daemon will write and
// where, the live confirmation once the hold key is pressed, and Skip
// beside Continue. Off Hyprland, Sway and Niri the snippet box gives way
// to the portal's outcome.
Item {
    id: root

    property var firstRun: null
    property var status: null

    readonly property bool supported: root.firstRun ? root.firstRun.snippetSupported : true
    readonly property string compositor: root.firstRun && root.firstRun.compositor.length > 0 ? root.firstRun.compositor : qsTr("No compositor")
    readonly property string configPath: root.firstRun && root.firstRun.configPath.length > 0 ? root.firstRun.configPath : "~/.config/dettivo/config.toml"
    readonly property bool omarchy: Theme.source === "omarchy"
    readonly property string hold: root.firstRun && root.firstRun.holdKey.length > 0 ? root.firstRun.holdKey : "F9"
    readonly property bool pressed: root.firstRun ? root.firstRun.pressed : false
    readonly property bool written: root.firstRun ? root.firstRun.snippetWritten : false
    readonly property bool sourced: root.firstRun ? root.firstRun.snippetSourced : false
    readonly property string subtitle: {
        if (!root.supported)
            return qsTr("%1 detected. There is no binding snippet for this desktop, so the keys go through the desktop portal.").arg(root.compositor);
        const where = root.omarchy ? qsTr("%1 on Omarchy detected.").arg(root.compositor) : qsTr("%1 detected.").arg(root.compositor);
        return qsTr("%1 These bindings go into a file your %2 config includes. On Omarchy, setup loads the bindings and reloads Hyprland.").arg(where).arg(root.compositor);
    }

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        subtitle: root.subtitle
        subtitleMaxWidth: Theme.space8 * 13 - Theme.space2
        title: qsTr("Keys")
    }

    Column {
        id: table
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: Theme.space7 + Theme.space3

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

            SectionLabel {
                anchors.left: parent.left
                anchors.leftMargin: parent.width - Theme.firstRunRunsColumn
                leftPadding: 0
                text: qsTr("Runs")
            }

            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                color: Theme.roleHairline
                height: Theme.hairlineWidth
            }
        }

        Repeater {
            model: root.firstRun ? root.firstRun.bindings : []

            delegate: BindingRow {
                required property var modelData
                action: modelData.action
                hint: modelData.hint
                keys: modelData.keys
                runs: modelData.runs
                width: table.width
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Bindings")
    }

    Rectangle {
        id: snippetBox
        anchors.left: parent.left
        anchors.top: table.bottom
        anchors.topMargin: Theme.space6 + Theme.space1
        border.color: Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: snippetColumn.implicitHeight + Theme.space5 * 2
        radius: Theme.radius
        width: Theme.firstRunSnippetWidth

        Column {
            id: snippetColumn
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4 + Theme.space1
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space5
            spacing: Theme.space2

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.supported ? (root.firstRun ? root.firstRun.snippetPath : "") : (root.firstRun ? root.firstRun.portalLine : "")
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.supported && root.firstRun ? root.firstRun.snippetText.trim() : ""
                visible: text.length > 0
                width: parent.width
                wrapMode: Text.Wrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text.length > 0 ? text : qsTr("Snippet")
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: root.supported ? qsTr("Bindings file") : qsTr("Portal bindings")
    }

    Rectangle {
        id: pressPanel
        anchors.right: parent.right
        anchors.top: snippetBox.top
        border.color: Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: pressColumn.implicitHeight + Theme.space5 * 2
        radius: Theme.radius
        width: Theme.firstRunPanelWidth

        Column {
            id: pressColumn
            anchors.left: parent.left
            anchors.leftMargin: Theme.space5
            anchors.right: parent.right
            anchors.rightMargin: Theme.space5
            anchors.top: parent.top
            anchors.topMargin: Theme.space5
            spacing: Theme.space3

            SectionLabel {
                bottomPadding: 0
                leftPadding: 0
                text: root.pressed ? qsTr("%1 pressed").arg(root.hold) : qsTr("Press %1 now").arg(root.hold)
                topPadding: 0
            }

            Row {
                spacing: Theme.space4

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    color: root.pressed ? Theme.roleAccent : Theme.roleFaintText
                    height: Theme.space2 + Theme.space1
                    radius: Theme.radius
                    width: height
                }

                Waveform {
                    anchors.verticalCenter: parent.verticalCenter
                    barColor: root.pressed ? Theme.roleAccent : Theme.roleFaintText
                    barCount: 10
                    floorLevel: root.pressed ? 0.35 : 0.12
                    height: Theme.space5
                    levels: root.status ? root.status.levels : []
                }
            }

            Text {
                color: root.pressed ? Theme.roleText : Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: root.pressed && root.firstRun ? root.firstRun.pressLine : qsTr("Waiting for the key.")
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: {
                    if (root.firstRun && root.firstRun.keysError.length > 0)
                        return root.firstRun.keysError;
                    if (!root.supported)
                        return root.firstRun ? root.firstRun.portalLine : "";
                    if (root.written && root.sourced)
                        return qsTr("Bindings loaded by %1. Hold the key to test your microphone.").arg(root.compositor);
                    if (root.written)
                        return qsTr("Bindings written. Add %1 to %2 and reload.").arg(root.firstRun ? root.firstRun.includeLine : "").arg(root.firstRun ? root.firstRun.mainConfigPath : "");
                    return qsTr("Continue sets up the shortcuts. Omarchy activates them automatically.");
                }
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Key check")
    }

    FirstRunFooter {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        note: qsTr("Prefer to do this by hand? Skip and edit %1. This screen never comes back once bindings exist.").arg(root.configPath)
        noteRich: qsTr("Prefer to do this by hand? Skip and edit %1. This screen never comes back once bindings exist.").arg("<font color=\"" + Theme.roleText + "\">" + Html.escaped(root.configPath) + "</font>")
        primary: root.supported && root.firstRun && root.firstRun.keysError.length > 0 ? qsTr("Retry shortcut setup") : qsTr("Continue")
        secondary: qsTr("Skip")
        onPrimaryTriggered: {
            if (root.firstRun)
                root.firstRun.next();
        }
        onSecondaryTriggered: {
            if (root.firstRun)
                root.firstRun.skip();
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Keys")
}
