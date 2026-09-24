import QtQuick
import Dettivo

// The rows under the Models table: the engine backends, the language
// model engine's limits, the diarization engine's threads, the idle
// unload of the language model, the engine directory and the catalogue
// switches, on the settings pattern.
Column {
    id: root

    property var settings: null

    spacing: 0

    SettingsGroup {
        title: qsTr("Engines")

        SettingRow {
            choices: ["auto", "vulkan", "cpu"]
            hint: qsTr("auto takes Vulkan when the model loads there")
            key: "engines.whisper.backend"
            kind: "choice"
            label: qsTr("Whisper backend")
            settings: root.settings
        }

        SettingRow {
            choices: ["auto", "vulkan", "cpu"]
            hint: qsTr("the same for Parakeet")
            key: "engines.parakeet.backend"
            kind: "choice"
            label: qsTr("Parakeet backend")
            settings: root.settings
        }

        SettingRow {
            choices: ["auto", "vulkan", "cpu"]
            hint: qsTr("falls back to the CPU when the model does not fit")
            key: "engines.llm.backend"
            kind: "choice"
            label: qsTr("Language model backend")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("tokens the language model is loaded with")
            key: "engines.llm.context_length"
            label: qsTr("Context length")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the most tokens one rewrite may generate")
            key: "engines.llm.max_tokens"
            label: qsTr("Max tokens")
            settings: root.settings
        }

        SettingRow {
            choices: ["auto", "cpu", "cuda"]
            hint: qsTr("auto takes CUDA when its runtime loads, else CPU with the reason")
            key: "engines.diarize.backend"
            kind: "choice"
            label: qsTr("Diarization backend")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("threads the diarization engine runs on; 0 is the core count, at most four")
            key: "engines.diarize.threads"
            label: qsTr("Diarization threads")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds the language model stays warm")
            key: "engines.llm_idle_seconds"
            label: qsTr("Unload the language model after")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("searched for engine binaries before PATH")
            key: "engines.directory"
            label: qsTr("Engine directory")
            placeholder: qsTr("the daemon's own directory")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Catalogue")

        SettingRow {
            hint: qsTr("downloads that may run at once")
            key: "models.max_concurrent_downloads"
            label: qsTr("Parallel downloads")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("re-hash unverified models at start; a mismatch quarantines")
            key: "models.verify_on_start"
            kind: "switch"
            label: qsTr("Verify on start")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("a catalogue file instead of the built-in one")
            key: "models.catalogue_file"
            label: qsTr("Catalogue file")
            placeholder: qsTr("built-in catalogue")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the Parakeet model a provider switch picks")
            key: "speech.parakeet_model_id"
            label: qsTr("Parakeet default")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Engine keys")
}
