import QtQuick
import Dettivo

// General: the daemon, the microphone, the dictation session, the data
// locations and the recording pill, on the settings pattern.
SettingsPage {
    id: root

    section: "general"
    subtitle: qsTr("Choose your microphone, dictation language and recording display. Add names and jargon in Vocabulary.")

    SettingsGroup {
        title: qsTr("Daemon")

        SettingRow {
            choices: ["error", "warn", "info", "debug", "trace"]
            hint: qsTr("journal level; never transcript text")
            key: "daemon.log_level"
            kind: "choice"
            label: qsTr("Log level")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds a graceful stop may take")
            key: "daemon.shutdown_timeout_ms"
            label: qsTr("Shutdown timeout")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Microphone")

        SettingRow {
            hint: qsTr("PipeWire node; empty follows the default source")
            key: "audio.input_device"
            label: qsTr("Input device")
            placeholder: qsTr("default source")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds between level samples")
            key: "audio.level_interval_ms"
            label: qsTr("Level interval")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Dictation")

        SettingRow {
            kind: "choice"
            segmentLimit: 0
            choices: SettingsUi.languageCodes
            choiceLabels: SettingsUi.languageLabels
            hint: qsTr("Language support depends on your speech model. Automatic detection is recommended.")
            key: "dictation.language"
            label: qsTr("Language")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds before a take stops itself")
            key: "dictation.max_duration_seconds"
            label: qsTr("Longest take")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("a take whose peak stays under this is silence")
            key: "dictation.silence_peak_threshold"
            label: qsTr("Silence threshold")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Locations")

        SettingRow {
            hint: qsTr("history and models; empty means $XDG_DATA_HOME/dettivo")
            key: "paths.data_dir"
            label: qsTr("Data directory")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("empty means <data_dir>/models")
            key: "paths.models_dir"
            label: qsTr("Models directory")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Recording pill")

        SettingRow {
            hint: qsTr("dettivo-osd outside Omarchy")
            visible: root.advanced || Theme.source !== "omarchy"
            key: "osd.enabled"
            kind: "switch"
            label: qsTr("Show the pill")
            settings: root.settings
        }

        SettingRow {
            choices: ["auto", "layer_shell", "window"]
            hint: qsTr("layer shell where the compositor offers one, else a window")
            key: "osd.host"
            kind: "choice"
            label: qsTr("Host")
            settings: root.settings
        }

        SettingRow {
            choices: ["top", "bottom", "top_left", "top_right", "bottom_left", "bottom_right"]
            hint: qsTr("the edge or corner it anchors to")
            key: "osd.position"
            kind: "choice"
            label: qsTr("Position")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("pixels from the anchored edges")
            key: "osd.margin"
            label: qsTr("Margin")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("focused, or an output name such as DP-3")
            key: "osd.monitor"
            label: qsTr("Monitor")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds Inserted and Copied stay")
            key: "osd.hide_after_ms"
            label: qsTr("Hide after")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds an error stays")
            key: "osd.error_hide_after_ms"
            label: qsTr("Error hide after")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the bars follow the microphone while listening")
            key: "osd.show_level"
            kind: "switch"
            label: qsTr("Show the level")
            settings: root.settings
        }

        SettingRow {
            choices: ["full", "reduced"]
            hint: qsTr("reduced holds the bars still and cuts between states")
            key: "osd.motion"
            kind: "choice"
            label: qsTr("Motion")
            settings: root.settings
        }
    }

    SettingsGroup {
        visible: root.advanced || Theme.source === "omarchy"
        title: qsTr("Omarchy plugin")

        SettingRow {
            choices: ["waveform", "dot"]
            hint: qsTr("the mark in the bar")
            key: "omarchy.glyph"
            kind: "choice"
            label: qsTr("Bar glyph")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the bars follow the live level while recording")
            key: "omarchy.level_meter"
            kind: "switch"
            label: qsTr("Level meter")
            settings: root.settings
        }

        SettingRow {
            choices: ["panel", "service", "off"]
            choiceLabels: [qsTr("Panel"), qsTr("Overlay"), qsTr("Hidden")]
            hint: qsTr("Show recording status in the Omarchy panel, a separate overlay, or hide it")
            key: "omarchy.osd"
            kind: "choice"
            label: qsTr("Recording pill")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the label beside Open Dettivo, in Hyprland's chord notation")
            key: "omarchy.open_shortcut"
            label: qsTr("Open shortcut")
            placeholder: qsTr("SUPER SHIFT, D")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("how many the panel lists")
            key: "omarchy.history_items"
            label: qsTr("Recent dictations")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
