import QtQuick
import DettivoStyle as Style
import Dettivo

// The 340 px panel the Omarchy bar widget opens (omarchy-bar-panel.png):
// the state header, the mode segment, Dictate and the meeting action,
// engine, Enhanced and insertion target as three key-value rows, the last
// three items, and Open Dettivo with its shortcut. The hint state stands
// in for all of it when the daemon or the module is missing. The plugin
// wires the signals to the `dettivo` command; this item owns the layout,
// the texts and the accessible names.
Rectangle {
    id: root

    // The item's `state`: idle, recording, transcribing, inserted, meeting,
    // unavailable or hint.
    // For the hint state: "install" (the module or the binary is missing)
    // or "upgrade" (the daemon is older than the plugin needs).
    property string hint: "install"
    // The version facts the hint names: what is installed and what is needed.
    property string hintDetail: ""
    // The inserted target and the first words, for the inserted header.
    property string target: ""
    property string words: ""
    // The meeting timer ("00:23:41") while meeting.
    property string elapsed: ""
    // The mode segment.
    property int modeIndex: 0
    // The three fact rows.
    property string engine: ""
    property string enhanced: ""
    property string insertTarget: ""
    // The last items: [{ time, title, app }].
    property var recent: []
    // The shortcut label beside Open Dettivo.
    property string shortcut: ""
    property bool reducedMotion: false

    signal dictate
    signal stopDictation
    signal cancelDictation
    signal startMeeting
    signal stopMeeting
    signal modeSelected(int index)
    signal openDettivo
    signal openItem(int index)

    readonly property var knownStates: ["idle", "recording", "transcribing", "inserted", "meeting", "unavailable", "hint"]
    readonly property string effectiveState: knownStates.indexOf(root.state) >= 0 ? root.state : "idle"
    readonly property bool recording: root.effectiveState === "recording"
    readonly property bool transcribing: root.effectiveState === "transcribing"
    readonly property bool meeting: root.effectiveState === "meeting"
    readonly property bool hinting: root.effectiveState === "hint"
    readonly property bool unavailable: root.effectiveState === "unavailable"
    readonly property bool live: !root.hinting && !root.unavailable
    // The status sentence the header shows.
    readonly property string sentence: {
        switch (root.effectiveState) {
        case "recording":
            return qsTr("Listening · release to insert");
        case "transcribing":
            return qsTr("Transcribing");
        case "inserted":
            return root.target.length > 0 ? qsTr("Inserted into %1").arg(root.target) : qsTr("Inserted");
        case "meeting":
            return root.elapsed.length > 0 ? qsTr("Meeting recording · %1").arg(root.elapsed) : qsTr("Meeting recording");
        case "unavailable":
            return qsTr("Daemon unavailable.");
        case "hint":
            return root.hint === "upgrade" ? qsTr("Upgrade Dettivo") : qsTr("Install Dettivo");
        default:
            return qsTr("Ready.");
        }
    }
    // The line under the sentence in the hint and unavailable states.
    readonly property string reason: {
        if (root.unavailable)
            return qsTr("systemctl --user start dettivod.socket");
        if (root.hinting)
            return root.hint === "upgrade" ? qsTr("yay -S dettivo-bin · %1").arg(root.hintDetail) : qsTr("yay -S dettivo-bin · the shared Dettivo module is missing");
        return "";
    }
    readonly property string glyphState: {
        switch (root.effectiveState) {
        case "recording":
            return "listening";
        case "transcribing":
            return "transcribing";
        case "meeting":
            return "meeting";
        default:
            return "idle";
        }
    }

    state: "idle"
    states: [
        State {
            name: "idle"
        },
        State {
            name: "recording"
        },
        State {
            name: "transcribing"
        },
        State {
            name: "inserted"
        },
        State {
            name: "meeting"
        },
        State {
            name: "unavailable"
        },
        State {
            name: "hint"
        }
    ]

    implicitWidth: Theme.barPanelWidth
    implicitHeight: column.implicitHeight
    color: Theme.roleSurface
    border.width: Theme.hairlineWidth
    border.color: Theme.roleBorder
    radius: Theme.radius

    Column {
        id: column
        width: parent.width
        spacing: 0

        BarPanelHeader {
            width: parent.width
            panel: root
        }

        Item {
            id: body
            visible: root.live
            width: parent.width
            height: visible ? bodyColumn.implicitHeight : 0

            Column {
                id: bodyColumn
                width: parent.width
                spacing: 0

                SegmentedControl {
                    id: modes
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width - Theme.rowPaddingX * 2
                    currentIndex: root.modeIndex
                    label: qsTr("Dictation mode")
                    model: [qsTr("Raw"), qsTr("Polish"), qsTr("Enhanced")]
                    onSelected: index => root.modeSelected(index)
                }

                Item {
                    width: parent.width
                    height: Theme.space3
                }

                Row {
                    id: actions
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width - Theme.rowPaddingX * 2
                    spacing: Theme.space2

                    Style.Button {
                        id: dictateButton
                        width: (actions.width - actions.spacing) / 2
                        leftPadding: Theme.space2
                        rightPadding: Theme.space2
                        spacing: Theme.space2
                        highlighted: root.recording
                        icon.name: root.recording ? "stop" : "mic"
                        text: root.recording ? qsTr("Stop") : qsTr("Dictate")
                        enabled: !root.transcribing
                        onClicked: root.recording ? root.stopDictation() : root.dictate()
                    }

                    Style.Button {
                        id: meetingButton
                        width: (actions.width - actions.spacing) / 2
                        leftPadding: Theme.space2
                        rightPadding: Theme.space2
                        spacing: Theme.space2
                        highlighted: root.meeting
                        icon.name: root.meeting ? "stop" : (root.recording ? "close" : "meeting")
                        text: root.meeting ? qsTr("Stop meeting") : (root.recording ? qsTr("Cancel") : qsTr("Start meeting"))
                        onClicked: {
                            if (root.meeting)
                                root.stopMeeting();
                            else if (root.recording)
                                root.cancelDictation();
                            else
                                root.startMeeting();
                        }
                    }
                }

                Item {
                    width: parent.width
                    height: Theme.space3
                }

                BarPanelFacts {
                    width: parent.width
                    panel: root
                }

                BarPanelRecent {
                    width: parent.width
                    panel: root
                }
            }
        }

        Rectangle {
            id: footer
            width: parent.width
            height: Theme.rowHeight
            color: footerHover.hovered ? Theme.roleHoverFill : "transparent"

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                height: Theme.hairlineWidth
                color: Theme.roleHairline
            }

            Text {
                anchors.left: parent.left
                anchors.leftMargin: Theme.rowPaddingX
                anchors.verticalCenter: parent.verticalCenter
                text: root.hinting ? qsTr("Open install guide") : qsTr("Open Dettivo")
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
            }

            Text {
                id: shortcutLabel
                visible: root.shortcut.length > 0 && !root.hinting
                anchors.right: parent.right
                anchors.rightMargin: Theme.rowPaddingX
                anchors.verticalCenter: parent.verticalCenter
                text: root.shortcut
                color: Theme.roleFaintText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeLabelSize
                font.capitalization: Font.AllUppercase
                font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Shortcut")
                Accessible.description: text
            }

            HoverHandler {
                id: footerHover
            }

            TapHandler {
                onTapped: root.openDettivo()
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.openDettivo();
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.Button
            Accessible.name: root.hinting ? qsTr("Open install guide") : qsTr("Open Dettivo")
            Accessible.onPressAction: root.openDettivo()
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Dettivo panel")
    Accessible.description: root.effectiveState
}
