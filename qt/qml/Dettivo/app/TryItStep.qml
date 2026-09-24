pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Try it (first-run-3-try-it.png): hold the key, speak, and the words land
// in the large field through the real insertion chain (the self-target
// allowance is armed while this step is on screen); the result names the
// backend, the stop-to-insert time and the mode. Open config.toml, Back
// and Done finish the flow.
Item {
    id: root

    property var firstRun: null
    property var status: null

    readonly property string hold: root.firstRun && root.firstRun.holdKey.length > 0 ? root.firstRun.holdKey : "F9"
    readonly property string dictationState: root.firstRun ? root.firstRun.dictationState : "idle"
    readonly property bool listening: root.dictationState === "recording"
    readonly property bool known: root.firstRun ? root.firstRun.resultKnown : false
    // The take's insertion outcome (`inserted`, `copied_to_clipboard`,
    // `failed`); the label, the explanation and the clipboard claim all
    // derive from it, never from the mere presence of a result.
    readonly property string outcome: root.known && root.firstRun ? root.firstRun.resultOutcome : ""
    readonly property bool inserted: root.outcome === "inserted"
    readonly property bool onClipboard: root.outcome === "copied_to_clipboard"
    readonly property string engine: root.firstRun && root.firstRun.engineLine.length > 0 ? root.firstRun.engineLine : qsTr("speech")
    readonly property string stateLabel: {
        switch (root.dictationState) {
        case "recording":
            return qsTr("Listening");
        case "transcribing":
            return qsTr("Transcribing");
        case "inserting":
            return qsTr("Inserting");
        default:
            if (root.inserted)
                return qsTr("Inserted");
            if (root.onClipboard)
                return qsTr("On the clipboard");
            return root.known ? qsTr("Not inserted") : qsTr("Idle");
        }
    }
    readonly property string reason: root.known && root.firstRun ? root.firstRun.resultReason : ""
    readonly property string configPath: root.firstRun && root.firstRun.configPath.length > 0 ? root.firstRun.configPath : "~/.config/dettivo/config.toml"
    readonly property string closingLine: qsTr("Meetings, history search, the CLI and MCP are ready too. Everything you just set lives in %1.")

    Component.onCompleted: {
        if (root.firstRun)
            root.firstRun.setAllowance(true);
        field.forceActiveFocus();
    }

    Component.onDestruction: {
        if (root.firstRun)
            root.firstRun.setAllowance(false);
    }

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        title: qsTr("Try it")
    }

    // The line under the title carries the key as a cap, the way the
    // artboard draws it.
    KeySentence {
        id: sentence
        anchors.left: parent.left
        anchors.top: header.bottom
        anchors.topMargin: Theme.space2
        key: root.hold
        lead: qsTr("Hold")
        tail: qsTr(", say a sentence, let go.")
    }

    Rectangle {
        id: panel
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: sentence.bottom
        anchors.topMargin: Theme.space7 + Theme.space2 + Theme.space1
        border.color: Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: Theme.space8 * 6 + Theme.space6 + Theme.space1
        radius: Theme.radius

        Rectangle {
            id: dot
            anchors.left: parent.left
            anchors.leftMargin: Theme.space6 + Theme.space1
            anchors.top: parent.top
            anchors.topMargin: Theme.space8 + Theme.space2
            color: root.listening ? Theme.roleAccent : Theme.roleFaintText
            height: Theme.space2 + Theme.space1 + Theme.space1
            radius: Theme.radius
            width: height
        }

        Waveform {
            anchors.left: dot.right
            anchors.leftMargin: Theme.space5
            anchors.verticalCenter: dot.verticalCenter
            barColor: root.listening ? Theme.roleAccent : Theme.roleFaintText
            barCount: 38
            height: Theme.space7 + Theme.space3
            levels: root.status ? root.status.levels : []
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.rightMargin: Theme.space6 + Theme.space1
            anchors.verticalCenter: dot.verticalCenter
            bottomPadding: 0
            leftPadding: 0
            text: qsTr("%1 · %2").arg(root.stateLabel).arg(root.engine)
            topPadding: 0
        }

        Rectangle {
            id: fieldFrame
            anchors.left: parent.left
            anchors.leftMargin: Theme.space6 + Theme.space1
            anchors.right: parent.right
            anchors.rightMargin: Theme.space6 + Theme.space1
            anchors.top: dot.bottom
            anchors.topMargin: Theme.space8
            border.color: field.activeFocus ? Qt.alpha(Theme.roleAccent, Theme.stateFocusBorderAlpha) : Theme.roleBorder
            border.width: Theme.stateNormalBorderWidth
            color: Theme.roleRaisedSurface
            height: Theme.firstRunFieldHeight
            radius: Theme.radius

            TextArea {
                id: field
                anchors.fill: parent
                anchors.margins: Theme.space4 + Theme.space1
                font.pixelSize: Theme.typeHeadingSize
                placeholderText: qsTr("Your words land here.")
                wrapMode: TextEdit.Wrap
                Accessible.name: qsTr("Try it field")

                // The sample text is set once, never bound: a binding would
                // wipe the words the chain typed on the next fact change.
                Component.onCompleted: {
                    if (root.firstRun && root.firstRun.sampleText.length > 0)
                        field.text = root.firstRun.sampleText;
                }
            }
        }

        Row {
            anchors.left: fieldFrame.left
            anchors.right: fieldFrame.right
            anchors.top: fieldFrame.bottom
            anchors.topMargin: Theme.space5 + Theme.space1
            spacing: Theme.space6

            Repeater {
                model: [
                    {
                        "label": qsTr("inserted via"),
                        "value": root.known && root.firstRun ? root.firstRun.insertedVia : "—"
                    },
                    {
                        "label": qsTr("stop to insert"),
                        "value": root.known && root.firstRun ? root.firstRun.stopToInsert : "—"
                    },
                    {
                        "label": qsTr("mode"),
                        "value": root.known && root.firstRun ? root.firstRun.modeLine : "—"
                    }
                ]

                delegate: Item {
                    id: fact
                    required property var modelData
                    height: Theme.space7 + Theme.space1
                    width: (fieldFrame.width - Theme.space6 * 2) / 3

                    Rectangle {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        color: Theme.roleHairline
                        height: Theme.hairlineWidth
                    }

                    Text {
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        color: Theme.roleText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.typeBodySize
                        font.weight: Theme.typeEmphasisWeight
                        text: fact.modelData.label
                        Accessible.role: Accessible.StaticText
                        Accessible.name: fact.modelData.label
                    }

                    Text {
                        anchors.bottom: parent.bottom
                        anchors.right: parent.right
                        color: Theme.roleMutedText
                        font.family: Theme.fontFamily
                        font.features: Theme.typeTabularNumerals
                        font.pixelSize: Theme.typeBodySize
                        text: fact.modelData.value
                        Accessible.role: Accessible.StaticText
                        Accessible.name: fact.modelData.label + ": " + fact.modelData.value
                    }
                }
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Try it panel")
    }

    Text {
        id: explain
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: panel.bottom
        anchors.topMargin: Theme.space7
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: {
            if (root.firstRun && !root.firstRun.micAvailable)
                return root.firstRun.micLine;
            if (root.onClipboard && root.reason.length > 0)
                return qsTr("Not typed: %1. The words are on the clipboard, paste them into the field.").arg(root.reason);
            if (root.onClipboard)
                return qsTr("The words are on the clipboard: paste them into the field.");
            if (root.known && !root.inserted && root.reason.length > 0)
                return qsTr("Not inserted: %1. The take is kept in History.").arg(root.reason);
            if (root.known && !root.inserted)
                return qsTr("Not inserted. The take is kept in History.");
            return qsTr("That works in any app that takes keyboard input. Terminals get Ctrl+Shift+V pastes when a backend needs the clipboard.");
        }
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: explain.bottom
        anchors.topMargin: Theme.space3
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.closingLine.arg("<font color=\"" + Theme.roleText + "\">" + Html.escaped(root.configPath) + "</font>")
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.closingLine.arg(root.configPath)
    }

    FirstRunFooter {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        leftAction: qsTr("Open config.toml")
        primary: qsTr("Done")
        secondary: qsTr("Back")
        onLeftTriggered: {
            if (root.firstRun)
                root.firstRun.openConfig();
        }
        onPrimaryTriggered: {
            if (root.firstRun)
                root.firstRun.finish();
        }
        onSecondaryTriggered: {
            if (root.firstRun)
                root.firstRun.back();
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Try it")
}
