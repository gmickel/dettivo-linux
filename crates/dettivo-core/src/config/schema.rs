//! The configuration schema: every key, its type and its default, plus the
//! commented default file `dettivo config print-default` emits. The two
//! are checked against each other in tests so `docs/config.md`, the file
//! and the code cannot drift apart silently.

use std::collections::BTreeMap;

use dettivo_proto::capabilities::IpcMode;
use serde::{Deserialize, Serialize};

pub use crate::config::audio_schema::{Audio, Diarization, MeetingAnalysis, Meetings};
pub use crate::config::import_schema::{Transcribe, Transfer};
pub use crate::config::omarchy_schema::{Omarchy, OmarchyGlyph, OmarchyOsd};
pub use crate::config::polish_schema::{Llm, LlmProviderChoice, Polish};
pub use crate::config::rest_schema::Rest;
pub use crate::config::speech_schema::{
    DiarizeBackend, DiarizeEngineSection, EngineBackend, EngineSection, Engines, LlmEngineSection,
    Models, Speech,
};

pub use crate::config::default_toml::DEFAULT_TOML;

/// The whole `config.toml`. Every field has a default, so a missing file
/// and an empty file are both the default configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// `[daemon]`: process behaviour.
    pub daemon: Daemon,
    /// `[ipc]`: the socket and its authentication.
    pub ipc: Ipc,
    /// `[paths]`: where data lives.
    pub paths: Paths,
    /// `[insert]`: the text insertion chain (ADR 0007).
    pub insert: Insert,
    /// `[qa]`: the QA rig switch (ADR 0011).
    pub qa: Qa,
    /// `[engines]`: engine processes (ADR 0003).
    pub engines: Engines,
    /// `[speech]`: the speech engine and model a session freezes at start.
    pub speech: Speech,
    /// `[audio]`: microphone capture (ADR 0006).
    pub audio: Audio,
    /// `[models]`: the catalogue and downloads.
    pub models: Models,
    /// `[dictation]`: the session's language, mode and raw text pipeline.
    pub dictation: Dictation,
    /// `[history]`: the dictation store, audio retention and artifacts.
    pub history: History,
    /// `[transcribe]`: the chunked long-audio pipeline (ADR 0022).
    pub transcribe: Transcribe,
    /// `[meetings]`: the two-stream capture, its checkpoint and artifacts
    /// (ADR 0027).
    pub meetings: Meetings,
    /// `[transfer]`: the upload limit.
    pub transfer: Transfer,
    /// `[osd]`: the recording pill outside Omarchy (`dettivo-osd`).
    pub osd: Osd,
    /// `[omarchy]`: the bar widget and the panel the Omarchy plugin draws.
    pub omarchy: Omarchy,
    /// `[hotkeys]`: the key chords, the daemon-side backend, media pause
    /// and feedback sounds.
    pub hotkeys: Hotkeys,
    /// `[mcp]`: the MCP server behind `dettivo mcp serve` (ADR 0019).
    pub mcp: Mcp,
    /// `[rest]`: the loopback REST shim (ADR 0028).
    pub rest: Rest,
    /// `[polish]`: the deterministic Polish pass and the app profiles
    /// (ADR 0023).
    pub polish: Polish,
    /// `[llm]`: the language model provider layer behind Enhanced
    /// (ADR 0023).
    pub llm: Llm,
}

/// `[mcp]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Mcp {
    /// `dettivo mcp config` writes the `DETTIVO_IPC_TOKEN` placeholder
    /// into every host entry without `--hardened`, and `dettivo mcp
    /// check` reports the mode; pairs with `[ipc] auth_mode = "peer_token"`.
    pub hardened: bool,
    /// Longest MCP message accepted or produced over stdio, in bytes.
    /// A longer message is refused with the cap named and the session
    /// continues.
    pub max_message_bytes: u64,
}

impl Default for Mcp {
    fn default() -> Self {
        Self {
            hardened: false,
            max_message_bytes: 2_000_000,
        }
    }
}

/// `[hotkeys]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkeys {
    /// `auto` uses the portal's GlobalShortcuts when the desktop answers
    /// and otherwise nothing (compositor bindings need no daemon side);
    /// `portal`, `evdev` or `none` pin it.
    pub backend: String,
    /// Push to talk: press starts, release stops (`SUPER CTRL, X` notation).
    pub hold: String,
    /// One press starts, the next stops.
    pub toggle: String,
    /// Drops the session.
    pub cancel: String,
    /// Inserts the last transcript again.
    pub reinsert: String,
    /// Pause playing MPRIS players while capturing and resume them after.
    pub pause_media: bool,
    /// Play a short sound on start, stop and error.
    pub sounds: bool,
    /// Input devices for the evdev backend; empty means every keyboard.
    pub evdev_devices: Vec<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            backend: "auto".into(),
            hold: "F9".into(),
            toggle: "SUPER CTRL, X".into(),
            cancel: "SUPER CTRL, Escape".into(),
            reinsert: "SUPER CTRL SHIFT, X".into(),
            pause_media: false,
            sounds: false,
            evdev_devices: Vec::new(),
        }
    }
}

/// `[insert]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Insert {
    /// `auto` runs the chain in order; a backend name pins that backend
    /// and an unavailable pin fails with its reason, never a fallback.
    pub backend: String,
    /// The paste keystroke per app id for the clipboard path; terminals
    /// default to `ctrl+shift+v`, everything else to `ctrl+v`.
    pub paste_keys: BTreeMap<String, String>,
    /// App ids that are terminals and take `ctrl+shift+v`.
    pub terminal_app_ids: Vec<String>,
    /// App ids of Dettivo's own windows; insertion into them fails with
    /// `target_is_self`.
    pub self_app_ids: Vec<String>,
    /// Pause between typed keys in milliseconds.
    pub inter_key_delay_ms: u64,
    /// Put the previous clipboard back after a clipboard insertion.
    pub restore_clipboard: bool,
    /// How long the restore waits for the target to take the paste.
    pub clipboard_restore_delay_ms: u64,
    /// How long after an insertion `insert.undo` may take it back.
    pub undo_window_ms: u64,
}

impl Default for Insert {
    fn default() -> Self {
        let text = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Self {
            backend: "auto".to_string(),
            paste_keys: BTreeMap::new(),
            terminal_app_ids: text(&[
                "foot",
                "footclient",
                "com.mitchellh.ghostty",
                "ghostty",
                "Alacritty",
                "alacritty",
                "kitty",
                "org.wezfurlong.wezterm",
                "wezterm",
                "xterm",
                "org.gnome.Console",
                "org.gnome.Terminal",
                "org.kde.konsole",
            ]),
            self_app_ids: text(&["dettivo", "dettivo-app", "dettivo-osd", "dettivo-sheet"]),
            inter_key_delay_ms: 2,
            restore_clipboard: true,
            clipboard_restore_delay_ms: 300,
            undo_window_ms: 5000,
        }
    }
}

/// `[daemon]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Daemon {
    /// The lowest level written to the journal.
    pub log_level: LogLevel,
    /// How long a graceful shutdown may take before the process exits anyway.
    pub shutdown_timeout_ms: u64,
}

impl Default for Daemon {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Info,
            shutdown_timeout_ms: 5000,
        }
    }
}

/// Log verbosity, from quietest to loudest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Failures only.
    Error,
    /// Failures and things that will become failures.
    Warn,
    /// Lifecycle: start, stop, config reloads, connections refused.
    Info,
    /// Per-request tracing without payloads.
    Debug,
    /// Everything, still without transcript text, audio or prompts.
    Trace,
}

impl LogLevel {
    /// The `tracing` filter directive for this level.
    pub fn as_directive(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

/// `[ipc]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ipc {
    /// `peer` (same uid) or `peer_token` (same uid plus the shared token).
    pub auth_mode: IpcMode,
    /// Socket path; empty means `$XDG_RUNTIME_DIR/dettivo/dettivo.sock`.
    pub socket: String,
    /// Token file for `peer_token` mode; empty means
    /// `$XDG_CONFIG_HOME/dettivo/ipc.token`.
    pub token_file: String,
    /// Longest request line accepted; a longer line is answered with
    /// `INVALID_PARAMS` and the connection stays open.
    pub max_line_bytes: u64,
}

impl Default for Ipc {
    fn default() -> Self {
        Self {
            auth_mode: IpcMode::Peer,
            socket: String::new(),
            token_file: String::new(),
            max_line_bytes: 1_048_576,
        }
    }
}

/// `[paths]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Paths {
    /// History and settings data; empty means `$XDG_DATA_HOME/dettivo`.
    pub data_dir: String,
    /// Model files; empty means `<data_dir>/models`.
    pub models_dir: String,
}

/// `[qa]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Qa {
    /// QA mode: the daemon accepts the rig's fake capture and insertion
    /// targets (ADR 0011). `DETTIVO_QA=1` turns it on without the file.
    pub mode: bool,
}

/// `[history]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct History {
    /// Keep the take of a finished dictation with its item.
    pub keep_audio: bool,
    /// Days after which retained audio is removed; 0 keeps it.
    pub audio_retention_days: u64,
    /// Items kept; the oldest beyond this are removed. 0 keeps every item.
    pub max_items: u64,
    /// `keep` (audio and `metadata.json`), `audio_only`, or `none`.
    pub artifacts: String,
    /// The database file; empty means `<data_dir>/dettivo.db`.
    pub db_path: String,
    /// The longest audio an import accepts, in seconds.
    pub max_import_seconds: u64,
}

impl Default for History {
    fn default() -> Self {
        Self {
            keep_audio: true,
            audio_retention_days: 30,
            max_items: 0,
            artifacts: "keep".into(),
            db_path: String::new(),
            max_import_seconds: 14_400,
        }
    }
}

/// `[dictation]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dictation {
    /// Language code or `auto`.
    pub language: String,
    /// `raw`, `deterministic_polish` (or its contract alias `polish`) or
    /// `enhanced`.
    pub mode: String,
    /// Words the engine is primed with (names, products).
    pub vocabulary: Vec<String>,
    /// Whole-word replacements applied after recognition (`{ from = "to" }`).
    pub replacements: std::collections::BTreeMap<String, String>,
    /// Spoken punctuation (`comma`, `period`, `new line`) becomes marks.
    pub spoken_punctuation: bool,
    /// File names, paths, URLs, email addresses, versions, dotted
    /// identifiers and backticked spans pass through the raw layer byte
    /// for byte: no sentence end on a dot inside one, no replacement
    /// inside one. Off makes every dot a sentence end again.
    pub protect_tokens: bool,
    /// The longest take before the session stops itself.
    pub max_duration_seconds: u64,
    /// A take whose peak stays under this is treated as silence.
    pub silence_peak_threshold: f64,
}

impl Default for Dictation {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            mode: "raw".into(),
            vocabulary: Vec::new(),
            replacements: std::collections::BTreeMap::new(),
            spoken_punctuation: true,
            protect_tokens: true,
            max_duration_seconds: 300,
            silence_peak_threshold: 0.01,
        }
    }
}

/// `[osd]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Osd {
    /// Whether `dettivo-osd` shows the pill at all; off exits with a notice.
    pub enabled: bool,
    /// `auto` (layer shell, then a window), `layer_shell` or `window`.
    pub host: String,
    /// `top`, `bottom`, `top_left`, `top_right`, `bottom_left`, `bottom_right`.
    pub position: String,
    /// Distance from the anchored edges in pixels.
    pub margin: u32,
    /// `focused` (the focused output on Hyprland, else the primary) or an
    /// output name such as `DP-3`.
    pub monitor: String,
    /// How long `inserted` and `copied` stay before the pill hides.
    pub hide_after_ms: u64,
    /// How long `error` stays before the pill hides.
    pub error_hide_after_ms: u64,
    /// Whether the bars follow the live level while listening.
    pub show_level: bool,
    /// `full` follows the desktop's reduced-motion preference; `reduced`
    /// holds the bars still and cuts between states.
    pub motion: String,
}

impl Default for Osd {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "auto".into(),
            position: "top".into(),
            margin: 24,
            monitor: "focused".into(),
            hide_after_ms: 1800,
            error_hide_after_ms: 4000,
            show_level: true,
            motion: "full".into(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_parses_to_the_default_config() {
        let parsed: Config = toml::from_str(DEFAULT_TOML).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn every_leaf_key_appears_uncommented_in_the_default_file() {
        fn check(section: &str, table: &serde_json::Value) {
            assert!(
                DEFAULT_TOML.contains(&format!("\n[{section}]\n")),
                "[{section}] missing"
            );
            for (key, value) in table.as_object().unwrap() {
                let listed = DEFAULT_TOML
                    .lines()
                    .any(|l| l.starts_with(&format!("{key} = ")));
                if !listed && value.is_object() {
                    check(&format!("{section}.{key}"), value);
                    continue;
                }
                assert!(listed, "{section}.{key} missing from DEFAULT_TOML");
            }
        }
        let value = serde_json::to_value(Config::default()).unwrap();
        for (section, table) in value.as_object().unwrap() {
            check(section, table);
        }
    }

    #[test]
    fn diarization_backend_choices_are_separate_from_ggml() {
        for backend in ["auto", "cpu", "cuda"] {
            let config: Config =
                toml::from_str(&format!("[engines.diarize]\nbackend = \"{backend}\"\n")).unwrap();
            assert_eq!(
                serde_json::to_value(config).unwrap()["engines"]["diarize"]["backend"],
                backend
            );
        }
        assert!(toml::from_str::<Config>("[engines.diarize]\nbackend = \"vulkan\"\n").is_err());
        for engine in ["whisper", "parakeet", "llm"] {
            assert!(
                toml::from_str::<Config>(&format!("[engines.{engine}]\nbackend = \"cuda\"\n"))
                    .is_err()
            );
        }
    }

    #[test]
    fn unknown_keys_and_bad_variants_are_rejected() {
        assert!(toml::from_str::<Config>("[daemon]\ncolour = 1\n").is_err());
        assert!(toml::from_str::<Config>("[daemon]\nlog_level = \"loud\"\n").is_err());
        assert!(toml::from_str::<Config>("[ipc]\nauth_mode = \"peer_token\"\n").is_ok());
        assert!(toml::from_str::<Config>("[engines.parakeet]\nbackend = \"cuda\"\n").is_err());
        let c: Config = toml::from_str("[engines.parakeet]\nbackend = \"cpu\"\n").unwrap();
        assert_eq!(c.engines.parakeet.backend, EngineBackend::Cpu);
        assert_eq!(c.engines.whisper.backend, EngineBackend::Auto);
    }
}
