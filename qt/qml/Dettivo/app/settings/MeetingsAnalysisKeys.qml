import QtQuick
import Dettivo

// The Analysis group of the Meetings route: the summary, decisions and
// action items a meeting gets once it finalised (ADR 0036), the provider
// and bounds of that pass, and the artifact policy a delete takes when
// none is named, on the settings pattern.
Column {
    id: root

    property var settings: null

    spacing: 0

    SettingsGroup {
        title: qsTr("Analysis")

        SettingRow {
            hint: qsTr("run the analysis after every finalisation")
            key: "meetings.analysis.auto"
            kind: "switch"
            label: qsTr("Run automatically")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds the whole analysis may take before it fails")
            key: "meetings.analysis.timeout_ms"
            label: qsTr("Timeout")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("characters of transcript per model call; longer transcripts are analysed in parts")
            key: "meetings.analysis.chunk_chars"
            label: qsTr("Chunk size")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("local, ollama, openai_compatible or auto; empty follows the language model provider")
            key: "meetings.analysis.provider"
            label: qsTr("Provider")
            placeholder: qsTr("follows [llm] provider")
            settings: root.settings
        }

        SettingRow {
            choices: ["transcript_only", "transcript_and_audio", "all"]
            hint: qsTr("what a delete removes when no policy is named")
            key: "meetings.delete_artifact_policy"
            kind: "choice"
            label: qsTr("Delete policy")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Analysis keys")
}
