pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Models (settings-models.png): the four selections (the dictation model,
// the meeting model, the Enhanced model, the idle unload) over `[speech]`,
// `[llm]` and `[engines]`, the table of every model with its state and
// actions, and the engine backends and catalogue keys below it.
SettingsPage {
    id: root

    property var table: null

    readonly property string speechKey: root.readSelection(root.revision)
    readonly property var speechChoices: root.table ? root.table.speechChoices : []
    readonly property var llmChoices: root.table ? root.table.llmChoices : []
    readonly property string refusal: root.table ? root.table.lastRefusal : ""

    section: "models"
    subtitle: root.table ? root.table.headline : ""

    function readSelection(revision) {
        if (!root.settings || revision < 0)
            return "";
        return root.settings.text("speech.provider") + "/" + root.settings.text("speech.model");
    }

    function readText(key, revision) {
        return root.settings && revision >= 0 ? root.settings.text(key) : "";
    }

    function keysOf(choices) {
        return choices.map(c => c.key);
    }

    function labelsOf(choices) {
        return choices.map(c => c.label);
    }

    trailing: Button {
        text: qsTr("Refresh catalogue")
        onClicked: {
            if (root.table)
                root.table.refresh();
        }
    }

    SettingRow {
        settings: root.settings
        key: "speech.model"
        label: qsTr("Dictation model")
        hint: qsTr("Download models from the catalogue below")
        kind: "choice"
        segmentLimit: 0
        choices: root.keysOf(root.speechChoices)
        choiceLabels: root.labelsOf(root.speechChoices)
        selection: root.speechKey
        resetter: () => {
            if (root.table && root.settings)
                root.table.select(root.settings.defaultValue("speech.provider"), root.settings.defaultValue("speech.model"));
        }
        writer: choice => {
            const parts = choice.split("/");
            if (root.table && parts.length === 2)
                root.table.select(parts[0], parts[1]);
        }
    }
    ModelSelections {
        settings: root.settings
        table: root.table
        includeEnhanced: true
    }
    SettingRow {
        settings: root.settings
        key: "engines.stt_idle_seconds"
        label: qsTr("Unload speech model after")
        hint: qsTr("seconds idle before GPU memory is released")
    }

    Item {
        height: Theme.space6
        width: parent.width
    }

    Item {
        height: Theme.space5 + Theme.space1
        width: parent.width

        SectionLabel {
            leftPadding: 0
            text: qsTr("Model")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8 * 7 - Theme.space3
            leftPadding: 0
            text: qsTr("Size")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8 * 9 + Theme.hairlineWidth
            leftPadding: 0
            text: qsTr("Backend")
        }

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8 * 11 + Theme.space7 + Theme.hairlineWidth
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
        id: rows
        spacing: 0
        width: parent.width

        Repeater {
            model: root.table

            delegate: ModelTableRow {
                id: row
                required property var model
                backend: row.model.backend
                detail: row.model.detail
                downloading: row.model.downloading
                modelId: row.model.modelId
                name: row.model.name
                progress: row.model.progress
                provider: row.model.provider
                ready: row.model.ready
                selected: row.model.selected
                size: row.model.size
                stateText: row.model.state
                warm: row.model.warm
                width: rows.width
                onCancel: root.table.cancel(row.model.provider, row.model.modelId)
                onDownload: root.table.download(row.model.provider, row.model.modelId)
                onRemove: root.table.remove(row.model.provider, row.model.modelId)
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Models table")
    }

    Text {
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.refusal
        topPadding: Theme.space3
        visible: root.refusal.length > 0
        width: parent.width
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Model action refused")
    }

    ModelsKeys {
        settings: root.settings
        width: parent.width
    }
}
