pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// One setting on the pattern (settings-hotkeys.png, the toggle panel):
// the label with the key's sentence under it and the key's name in the
// faint caption, the control for the key's type on the right, the ENV
// badge with the variable's name when the environment set the value
// (the control is then disabled), and the daemon's refusal inline under
// the control after a write it did not accept.
Item {
    id: root

    property var settings: null
    property string key: ""
    property string label: ""
    property string hint: ""
    // `switch`, `field`, `choice` or `none` (a row that carries its own control).
    property string kind: "field"
    property var choices: []
    property var choiceLabels: []
    property string placeholder: ""
    property int controlWidth: Theme.settingsControlWidth
    // Segments up to this many choices; a combo box beyond.
    property int segmentLimit: 5
    // The caption names the key after the hint; a narrow panel shows the hint alone.
    property bool showKey: SettingsUi.advanced
    property var writer: null
    property var resetter: null
    property string selection: valueText
    visible: SettingsUi.show(key)

    function commit(value) {
        if (root.writer)
            root.writer(value);
        else if (root.settings)
            root.settings.set(root.key, value);
    }
    property bool separator: true
    default property alias extra: controlSlot.data

    readonly property int revision: root.settings ? root.settings.revision : 0
    readonly property string valueText: root.readText(root.revision)
    readonly property var value: root.readValue(root.revision)
    readonly property string lockedBy: root.readLock(root.revision)
    readonly property string error: root.readError(root.errorRevision)
    readonly property string caption: !root.showKey ? root.hint : (root.hint.length > 0 ? qsTr("%1 · %2").arg(root.hint).arg(root.key) : root.key)
    property int errorRevision: 0
    readonly property var controlItem: control.item

    width: parent ? parent.width : implicitWidth
    implicitHeight: Math.max(Theme.settingsRowHeight, labels.implicitHeight + Theme.space3 * 2) + (root.error.length > 0 ? errorText.implicitHeight + Theme.space2 : 0)

    // Each reader takes the revision so the binding follows every refresh.
    function readText(revision) {
        return root.settings && revision >= 0 ? root.settings.text(root.key) : "";
    }

    function readValue(revision) {
        return root.settings && revision >= 0 ? root.settings.value(root.key) : undefined;
    }

    function readLock(revision) {
        return root.settings && revision >= 0 ? root.settings.lockedBy(root.key) : "";
    }

    function readError(revision) {
        return root.settings && revision >= 0 ? root.settings.error(root.key) : "";
    }

    Connections {
        target: root.settings
        function onErrorChanged(key) {
            if (key === root.key)
                root.errorRevision += 1;
        }
    }

    Column {
        id: labels
        anchors.left: parent.left
        anchors.right: controlSlot.left
        anchors.rightMargin: Theme.space5
        anchors.top: parent.top
        anchors.topMargin: Theme.space3
        spacing: Theme.space1

        Row {
            spacing: Theme.space3

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: root.label
                Accessible.role: Accessible.StaticText
                Accessible.name: root.label
            }

            Chip {
                accent: true
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("env · %1").arg(root.lockedBy)
                visible: root.lockedBy.length > 0
            }
        }

        Text {
            color: Theme.roleMutedText
            wrapMode: Text.WordWrap
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.caption
            width: labels.width
            Accessible.role: Accessible.StaticText
            Accessible.name: root.caption
        }
    }

    Item {
        id: controlSlot
        anchors.right: reset.left
        anchors.rightMargin: Theme.space2
        anchors.top: parent.top
        anchors.topMargin: (Math.max(Theme.settingsRowHeight, labels.implicitHeight + Theme.space3 * 2) - Theme.controlHeight) / 2
        height: Theme.controlHeight
        width: root.kind === "none" ? childrenRect.width : (root.kind === "switch" ? (root.controlItem ? root.controlItem.implicitWidth : root.controlWidth) : Math.max(root.controlWidth, root.controlItem ? root.controlItem.implicitWidth : 0))

        Loader {
            id: control
            anchors.fill: parent
            active: root.kind !== "none"
            sourceComponent: {
                switch (root.kind) {
                case "switch":
                    return switchControl;
                case "choice":
                    return choiceControl;
                case "none":
                    return null;
                default:
                    return fieldControl;
                }
            }
        }
    }

    SettingReset {
        id: reset
        resetter: root.resetter
        anchors.right: parent.right
        anchors.verticalCenter: controlSlot.verticalCenter
        key: root.key
        settings: root.settings
    }

    Text {
        id: errorText
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: controlSlot.bottom
        anchors.topMargin: Theme.space2
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        horizontalAlignment: Text.AlignRight
        text: root.error
        visible: root.error.length > 0
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("%1 refused").arg(root.key)
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
        visible: root.separator
    }

    Component {
        id: switchControl
        SettingSwitch {
            checked: root.value === true
            enabled: root.lockedBy.length === 0
            name: root.key
            onToggled: on => {
                root.commit(on);
            }
        }
    }

    Component {
        id: fieldControl
        SettingField {
            enabled: root.lockedBy.length === 0
            name: root.key
            placeholder: root.placeholder
            value: root.valueText
            onCommitted: text => {
                root.commit(text);
            }
        }
    }

    Component {
        id: choiceControl
        SettingChoice {
            choiceLabels: root.choiceLabels
            choices: root.choices
            current: root.selection
            enabled: root.lockedBy.length === 0
            name: root.key
            segmentLimit: root.segmentLimit
            onChosen: choice => {
                root.commit(choice);
            }
        }
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: qsTr("%1 row").arg(root.key)
}
