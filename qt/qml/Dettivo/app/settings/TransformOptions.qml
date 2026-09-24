pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

SettingsGroup {
    id: root
    property string key: "polish.transforms"
    property var settings: null
    readonly property int revision: settings ? settings.revision : 0
    readonly property var selected: settings && revision >= 0 ? settings.value("polish.transforms") || [] : []
    readonly property string lockedBy: settings && revision >= 0 ? settings.lockedBy("polish.transforms") : ""
    readonly property bool pending: settings && revision >= 0 ? settings.pending("polish.transforms") : false
    property int errorRevision: 0
    title: qsTr("Polish transforms")
    function toggle(option, enabled) {
        if (!settings || pending || lockedBy.length)
            return;
        settings.set("polish.transforms", enabled ? selected.concat([option]) : selected.filter(value => value !== option));
    }
    Connections {
        target: root.settings
        function onErrorChanged(key) {
            if (key === "polish.transforms")
                root.errorRevision++;
        }
    }
    Repeater {
        model: [
            {
                key: "fixGrammar",
                label: qsTr("Fix grammar"),
                hint: qsTr("Clean up spacing and sentence capitalization")
            },
            {
                key: "removeFillers",
                label: qsTr("Remove fillers"),
                hint: qsTr("Remove hesitation words such as um and uh")
            },
            {
                key: "smartPunctuation",
                label: qsTr("Smart punctuation"),
                hint: qsTr("Add sentence punctuation; code presets keep it off")
            }
        ]
        delegate: CheckBox {
            id: option
            required property var modelData
            text: modelData.label + " · " + modelData.hint
            checked: root.selected.indexOf(modelData.key) >= 0
            nextCheckState: function () {
                return checkState;
            }
            enabled: !root.pending && root.lockedBy.length === 0
            onClicked: root.toggle(modelData.key, !checked)
            Accessible.name: modelData.label
        }
    }
    Text {
        readonly property var custom: root.selected.filter(value => ["fixGrammar", "removeFillers", "smartPunctuation"].indexOf(value) < 0)
        visible: custom.length > 0 || root.lockedBy.length > 0
        text: root.lockedBy.length ? qsTr("Set by %1").arg(root.lockedBy) : qsTr("Additional configured transforms: %1").arg(custom.join(", "))
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    SettingReset {
        settings: root.settings
        key: "polish.transforms"
        busy: root.pending
    }
    Text {
        text: root.settings && root.errorRevision >= 0 ? root.settings.error("polish.transforms") : ""
        visible: text.length > 0
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    Accessible.role: Accessible.Grouping
    Accessible.name: title
}
