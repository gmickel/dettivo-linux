//! `system.capabilities` result: the macOS flag set (`docs/api/
//! dettivo-ipc-v1.md` section 6.2) plus the two Linux additions registered
//! in `docs/api/linux-deltas.md` — a `platform` block and a `speech` block
//! naming the adopted Windows provider-selection methods.

use serde::{Deserialize, Serialize};

pub use crate::capabilities_speech::{MeetingCaps, SpeechCaps, SpeechProviderCaps};
pub use crate::tier::Tier;

/// IPC auth mode, mirrored in `auth.ipc_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcMode {
    /// Peer uid/pid verification only, no token.
    Peer,
    /// Peer verification plus a shared token.
    PeerToken,
}

/// Auth capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthCaps {
    /// Current IPC auth mode.
    pub ipc_mode: IpcMode,
    /// Whether the REST shim requires a token (always `true` per contract).
    pub rest_token_required: bool,
}

/// A transcript export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// Plain text.
    Txt,
    /// Markdown.
    Md,
    /// JSON. Present in `dictation_export` on macOS; a Linux delta adds it
    /// to `meeting_export` too (`docs/api/linux-deltas.md`).
    Json,
    /// A ZIP archive of items with their audio (Linux addition, signalled
    /// by `history.archive_export`; never listed in the format scopes).
    Zip,
    /// SubRip subtitles.
    Srt,
    /// WebVTT subtitles.
    Vtt,
}

/// Export format scopes by history kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatCaps {
    /// Formats available for dictation export.
    pub dictation_export: Vec<ExportFormat>,
    /// Formats available for meeting export.
    pub meeting_export: Vec<ExportFormat>,
}

/// How captured/produced text may be inserted into the focused app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertMode {
    /// Insert the raw transcript.
    Raw,
    /// Insert an LLM-polished transcript.
    Polish,
    /// Copy to the clipboard only, no simulated paste.
    ClipboardOnly,
}

/// Binary transfer limits (section 8.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferCaps {
    /// Maximum bytes per `transfer.chunk`.
    pub chunk_max_bytes: u64,
    /// Maximum concurrent transfers.
    pub max_inflight: u32,
    /// Transfer idle timeout in seconds.
    pub timeout_seconds: u64,
}

/// Automation feature flags. `jobs` and `providers` are `false` on Linux:
/// `automation.jobs.*` and `automation.providers.*` are reserved here
/// (`docs/api/linux-deltas.md`), so the flags a client already branches on
/// say so without a separate signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationCaps {
    /// Whether automation jobs may send email.
    pub email_delivery: bool,
    /// Whether `automation.jobs.*` is implemented.
    pub jobs: bool,
    /// Whether `automation.providers.*` is implemented.
    pub providers: bool,
    /// Whether dictation macros are implemented.
    pub dictation_macros: bool,
    /// Whether dictation macro audit history is implemented.
    pub dictation_macro_audit: bool,
}

/// Knowledge-base feature flags. Both are `false`: `knowledge.*` is a
/// reserved namespace (`docs/api/linux-deltas.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCaps {
    /// Whether semantic search over history is implemented.
    pub semantic_search: bool,
    /// Whether cited question-answering is implemented.
    pub ask_with_citations: bool,
}

/// Retention feature flags. `delete` is true on Linux (`meetings.delete`
/// is implemented); `auto_delete` stays false while `retention.*` is a
/// reserved namespace (`docs/api/linux-deltas.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionCaps {
    /// Whether manual deletion is implemented.
    pub delete: bool,
    /// Whether policy-driven auto-deletion is implemented.
    pub auto_delete: bool,
}

/// Linux addition: which desktop transport is driving the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// A Wayland compositor session.
    Wayland,
    /// An X11 (or XWayland) session.
    X11,
}

/// Linux addition: platform identity and capability block, registered as a
/// delta in `docs/api/linux-deltas.md` (ADR 0008 consequences).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformCaps {
    /// Always `"linux"`.
    pub os: String,
    /// IPC transport, e.g. `"unix_socket"`.
    pub transport: String,
    /// Detected compositor name, when known (e.g. `"Hyprland"`).
    pub compositor: Option<String>,
    /// Wayland or X11/XWayland.
    pub session: SessionKind,
    /// Active text-insertion backend name.
    pub insertion_backend: String,
    /// Detected GPU vendor/driver string, when known.
    pub gpu: Option<String>,
    /// Linux addition (S-16): the hardware tier the benchmarks and the NFR
    /// targets are read by, `gpu` or `cpu`.
    pub tier: Tier,
    /// Why that tier: the Vulkan device, the engines' backends, or the
    /// `DETTIVO_FORCE_CPU=1` override.
    pub tier_reason: String,
}

/// The Linux addition that exposes the configuration file over the
/// contract (`config.*`); an empty list means the daemon has no file API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigCaps {
    /// The `config.*` method names the daemon implements.
    pub methods: Vec<String>,
}

impl ConfigCaps {
    /// The seven methods a Linux daemon implements (ADR 0009, ADR 0033).
    pub fn linux_methods() -> Self {
        Self {
            methods: [
                "config.get",
                "config.set",
                "config.unset",
                "config.validate",
                "config.path",
                "config.print_default",
                "config.keys",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Linux addition: names the insertion methods beyond `insert.perform`
/// this daemon implements, registered as a delta in
/// `docs/api/linux-deltas.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertCaps {
    /// The `insert.*` method names beyond `insert.perform`.
    pub methods: Vec<String>,
}

impl InsertCaps {
    /// The Linux additions: `insert.undo` and `insert.target` (ADR 0007),
    /// `insert.allow_self_target` (ADR 0024).
    pub fn linux_methods() -> Self {
        Self {
            methods: ["insert.undo", "insert.target", "insert.allow_self_target"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Linux addition: the history store's search and export abilities and
/// the `transcripts.*` methods Linux adds, registered in
/// `docs/api/linux-deltas.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryCaps {
    /// Full-text search (SQLite FTS5) backs `transcripts.search`.
    pub fts: bool,
    /// Semantic search; `false` until `knowledge.*` lands (`score` is null).
    pub semantic_search: bool,
    /// `transcripts.export` accepts format `zip` (items with their audio).
    pub archive_export: bool,
    /// The Linux-added method names under `transcripts.*`.
    pub methods: Vec<String>,
}

impl HistoryCaps {
    /// What a Linux daemon declares.
    pub fn linux() -> Self {
        Self {
            fts: true,
            semantic_search: false,
            archive_export: true,
            methods: [
                "transcripts.delete",
                "transcripts.rerun",
                "transcripts.stats",
                "transcripts.cancel",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Linux addition: the hotkey backend in force and the ones this daemon
/// could run, registered as a delta in `docs/api/linux-deltas.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HotkeyCaps {
    /// `portal`, `evdev` or `none`.
    pub backend: String,
    /// The backends available in this session right now.
    pub available: Vec<String>,
}

/// Linux addition: what the Polish layers offer (ADR 0023). The mode
/// identifiers are the ones `dictation.start` accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolishCaps {
    /// `raw`, `deterministic_polish`, `enhanced`.
    pub modes: Vec<String>,
    /// The built-in preset ids.
    pub presets: Vec<String>,
    /// The style ids.
    pub styles: Vec<String>,
    /// Whether `[polish] presets` custom presets are honoured.
    pub custom_presets: bool,
}

impl PolishCaps {
    /// The blocks this daemon declares: every mode, preset and style the
    /// language crate implements.
    pub fn new(modes: &[&str], presets: &[&str], styles: &[&str]) -> Self {
        Self {
            modes: modes.iter().map(|s| (*s).to_string()).collect(),
            presets: presets.iter().map(|s| (*s).to_string()).collect(),
            styles: styles.iter().map(|s| (*s).to_string()).collect(),
            custom_presets: true,
        }
    }
}

/// Linux addition: the language model provider layer behind Enhanced
/// (ADR 0023) and the local engine with its catalogue (ADR 0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmCaps {
    /// The provider ids this build carries.
    pub providers: Vec<String>,
    /// Whether the `local` provider would answer right now: the
    /// `dettivo-engine-llm` binary is found and `[llm] model` is on disk.
    pub local_available: bool,
    /// The `llm.*` methods this daemon answers.
    pub methods: Vec<String>,
}

impl LlmCaps {
    /// The block this daemon declares: the three providers, whether the
    /// local engine would answer, and the Linux methods.
    pub fn linux(local_available: bool) -> Self {
        Self {
            providers: ["local", "ollama", "openai_compatible"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            local_available,
            methods: [
                "llm.providers.list",
                "llm.endpoints.trust",
                "llm.endpoints.list",
                "llm.models.status",
                "llm.models.download",
                "llm.models.cancel",
                "llm.models.delete",
                "llm.engine.status",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        }
    }
}

/// Linux addition: the loopback REST shim (ADR 0028), registered as a
/// delta in `docs/api/linux-deltas.md`. `enabled` is whether a listener
/// is up inside the daemon right now, `port` the port it is bound to
/// (the configured one otherwise) and `bind` the address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestCaps {
    /// Whether the daemon hosts the listener right now.
    pub enabled: bool,
    /// The port the listener is bound to, or the configured port.
    pub port: u16,
    /// The bind address (loopback only).
    pub bind: String,
}

/// Full `system.capabilities` result: the macOS flag set plus the Linux
/// `platform`, `speech`, `config`, `insert`, `polish`, `llm`, `hotkeys`,
/// `meetings` and `rest` additions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Auth mode flags.
    pub auth: AuthCaps,
    /// Export format scopes.
    pub formats: FormatCaps,
    /// Supported insertion modes.
    pub insert_modes: Vec<InsertMode>,
    /// Binary transfer limits.
    pub transfers: TransferCaps,
    /// Automation feature flags.
    pub automation: AutomationCaps,
    /// Knowledge-base feature flags.
    pub knowledge: KnowledgeCaps,
    /// Whether meeting templates are implemented.
    pub meeting_templates: bool,
    /// Retention feature flags.
    pub retention: RetentionCaps,
    /// Linux addition: platform identity block.
    pub platform: PlatformCaps,
    /// Linux addition: adopted speech provider/selection methods.
    pub speech: SpeechCaps,
    /// Linux addition: the configuration file methods (ADR 0009).
    pub config: ConfigCaps,
    /// Linux addition: the insertion methods beyond `insert.perform` (ADR 0007).
    pub insert: InsertCaps,
    /// Linux addition: the history store's abilities and added methods.
    pub history: HistoryCaps,
    /// Linux addition: the Polish layers (ADR 0023).
    pub polish: PolishCaps,
    /// Linux addition: the language model provider layer (ADR 0023).
    pub llm: LlmCaps,
    /// Linux addition: the hotkey backend in force.
    pub hotkeys: HotkeyCaps,
    /// Linux addition: the meeting methods and checkpoint schema.
    pub meetings: MeetingCaps,
    /// Linux addition: the loopback REST shim (ADR 0028).
    pub rest: RestCaps,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_caps_names_adopted_windows_methods() {
        let caps = SpeechCaps::adopted_windows_methods();
        assert!(caps.methods.contains(&"speech.providers.list".to_string()));
        assert!(caps.methods.contains(&"speech.selection.get".to_string()));
        assert!(caps.methods.contains(&"speech.selection.set".to_string()));
        let caps = caps.with_providers(&[("whisper", true), ("parakeet", false)]);
        assert!(caps.providers["whisper"].meeting_capable);
        assert!(!caps.providers["parakeet"].meeting_capable);
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["providers"]["parakeet"]["meeting_capable"], false);
    }

    #[test]
    fn export_format_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ExportFormat::Json).unwrap(),
            "\"json\""
        );
    }
}
