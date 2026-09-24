import QtQuick
import Dettivo

Column {
    id: root
    property var settings: null
    property var table: null
    property bool includeEnhanced: false
    readonly property var speech: table ? table.speechChoices : []
    readonly property var language: table ? table.llmChoices : []
    readonly property int revision: settings ? settings.revision : 0
    readonly property string provider: settings && revision >= 0 ? settings.text("speech.provider") : ""
    width: parent ? parent.width : implicitWidth
    SettingRow {
        settings: root.settings
        key: "speech.meeting_model"
        label: qsTr("Meeting model")
        hint: root.provider === "whisper" ? qsTr("Download meeting models in Models") : qsTr("Select a Whisper meeting model. Parakeet remains your dictation model.")
        kind: "choice"
        segmentLimit: 0
        choices: [""].concat(root.speech.filter(c => c.key.split("/")[0] === "whisper").map(c => c.key.split("/")[1]))
        choiceLabels: [qsTr("Use dictation model")].concat(root.speech.filter(c => c.key.split("/")[0] === "whisper").map(c => c.label))
    }
    SettingRow {
        settings: root.settings
        key: root.includeEnhanced ? "llm.model" : "llm.analysis_model"
        label: root.includeEnhanced ? qsTr("Enhanced model") : qsTr("Analysis model")
        hint: qsTr("Local catalogue model; download models in Models")
        kind: "choice"
        segmentLimit: 0
        choices: root.includeEnhanced ? root.language.map(c => c.key) : [""].concat(root.language.map(c => c.key))
        choiceLabels: root.includeEnhanced ? root.language.map(c => c.label) : [qsTr("Use Enhanced model")].concat(root.language.map(c => c.label))
    }
    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Model selections")
}
