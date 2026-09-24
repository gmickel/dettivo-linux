import QtQuick
import Dettivo

// Polish: the mode and the deterministic pass over `[dictation]` and
// `[polish]`, then the language model providers behind Enhanced over
// `[llm]`; the rules and the app profiles are tables the file and
// `dettivo polish` edit, named here so nothing is hidden.
SettingsPage {
    id: root

    section: "polish"
    subtitle: qsTr("Choose how Dettivo cleans up your words before inserting them.")

    Text {
        readonly property string experiment: root.settings && root.revision >= 0 ? root.settings.text("llm.polish_experiment") : ""
        text: qsTr("Experimental polish model active: %1. Manage it in Advanced settings.").arg(experiment)
        visible: experiment.length > 0
        width: parent.width
        wrapMode: Text.WordWrap
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    SettingsGroup {
        title: qsTr("Mode")

        SettingRow {
            choiceLabels: [qsTr("Raw"), qsTr("Polish"), qsTr("Enhanced")]
            choices: ["raw", "deterministic_polish", "enhanced"]
            hint: qsTr("what runs after recognition")
            key: "dictation.mode"
            kind: "choice"
            label: qsTr("Dictation mode")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("comma, period, new line become marks")
            key: "dictation.spoken_punctuation"
            kind: "switch"
            label: qsTr("Spoken punctuation")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("whole words after recognition, from=to")
            key: "dictation.replacements"
            label: qsTr("Replacements")
            placeholder: qsTr("teh=the, dettivo=Dettivo")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("file names, paths, URLs and versions pass through untouched")
            key: "dictation.protect_tokens"
            kind: "switch"
            label: qsTr("Protect tokens")
            settings: root.settings
        }
    }

    TransformOptions {
        settings: root.settings
    }

    SettingsGroup {
        title: qsTr("Polish pass")

        SettingRow {
            choices: ["generic", "email", "code", "chat", "notes"]
            choiceLabels: [qsTr("General"), qsTr("Email"), qsTr("Code"), qsTr("Chat"), qsTr("Notes")]
            hint: qsTr("for an app without a profile")
            key: "polish.default_preset"
            kind: "choice"
            label: qsTr("Default preset")
            settings: root.settings
        }

        SettingRow {
            choices: ["asDictated", "formal", "casual", "veryCasual"]
            choiceLabels: [qsTr("As dictated"), qsTr("Formal"), qsTr("Casual"), qsTr("Very casual")]
            hint: qsTr("asDictated lets each preset pick")
            key: "polish.default_style"
            kind: "choice"
            label: qsTr("Default style")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Enhanced providers")

        SettingRow {
            choices: ["auto", "local", "ollama", "openai_compatible"]
            hint: qsTr("auto takes the first available")
            key: "llm.provider"
            kind: "choice"
            label: qsTr("Provider")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("detected through GET /api/tags")
            key: "llm.ollama_url"
            label: qsTr("Ollama URL")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("ollama pull <model>")
            key: "llm.ollama_model"
            label: qsTr("Ollama model")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("/v1/chat/completions is appended; a remote host needs trust")
            key: "llm.endpoint_url"
            label: qsTr("OpenAI-compatible endpoint")
            placeholder: qsTr("http://localhost:8080")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the model the endpoint is asked for")
            key: "llm.endpoint_model"
            label: qsTr("Endpoint model")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("mode 0600; the Secret Service item is read first")
            key: "llm.api_key_file"
            label: qsTr("API key file")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("remote endpoints you confirmed with dettivo llm trust")
            key: "llm.trusted_endpoints"
            label: qsTr("Trusted endpoints")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds the whole pass may take")
            key: "llm.timeout_ms"
            label: qsTr("Timeout")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("repair attempts inside the same budget")
            key: "llm.max_retries"
            label: qsTr("Retries")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Experimental models · advanced")

        SettingRow {
            hint: qsTr("a sideloaded fine-tune manifest; empty keeps the catalogue model")
            key: "llm.polish_experiment"
            label: qsTr("Polish experiment")
            placeholder: qsTr("current")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("empty means <models_dir>/polish-experiments")
            key: "llm.experiments_dir"
            label: qsTr("Experiments directory")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
