//! The `system.capabilities` answer: the contract's flag set with the
//! Linux additions, and the platform block detected from the session.

use std::path::Path;

use dettivo_core::config::polish_schema::{Preset, Style};
use dettivo_proto::capabilities::{
    AuthCaps, AutomationCaps, Capabilities, ConfigCaps, ExportFormat, FormatCaps, HistoryCaps,
    HotkeyCaps, InsertCaps, InsertMode, IpcMode, KnowledgeCaps, LlmCaps, MeetingCaps, PlatformCaps,
    PolishCaps, RestCaps, RetentionCaps, SessionKind, SpeechCaps, TransferCaps,
};
use dettivo_speech::tier::TierReport;

/// The transfer limits this daemon enforces (`transfers` block).
pub fn transfer_caps() -> TransferCaps {
    TransferCaps {
        chunk_max_bytes: 1_048_576,
        max_inflight: 4,
        timeout_seconds: 120,
    }
}

/// Builds the capability snapshot for this daemon. `insertion_backend` is
/// the backend the chain would use right now, `none` when nothing is
/// available; `hotkeys` names the hotkey backend in force;
/// `local_llm_available` says whether the local language model would
/// answer (ADR 0026); `tier` is the hardware tier the supervisor reports
/// (S-16).
pub fn capabilities(
    ipc_mode: IpcMode,
    insertion_backend: Option<String>,
    hotkeys: HotkeyCaps,
    local_llm_available: bool,
    rest: RestCaps,
    tier: &TierReport,
) -> Capabilities {
    Capabilities {
        auth: AuthCaps {
            ipc_mode,
            rest_token_required: true,
        },
        formats: FormatCaps {
            dictation_export: vec![ExportFormat::Txt, ExportFormat::Md, ExportFormat::Json],
            meeting_export: vec![
                ExportFormat::Txt,
                ExportFormat::Md,
                ExportFormat::Json,
                ExportFormat::Srt,
                ExportFormat::Vtt,
            ],
        },
        insert_modes: vec![
            InsertMode::Raw,
            InsertMode::Polish,
            InsertMode::ClipboardOnly,
        ],
        transfers: transfer_caps(),
        automation: AutomationCaps {
            email_delivery: false,
            jobs: false,
            providers: false,
            // No macros spec has landed on Linux: the six
            // `automation.macros.*` methods answer `NOT_IMPLEMENTED`, and
            // the flags say so (`docs/api/linux-deltas.md`).
            dictation_macros: false,
            dictation_macro_audit: false,
        },
        knowledge: KnowledgeCaps {
            semantic_search: false,
            ask_with_citations: false,
        },
        meeting_templates: false,
        retention: RetentionCaps {
            delete: true,
            auto_delete: false,
        },
        platform: platform(|k| std::env::var(k).ok(), insertion_backend, tier),
        speech: SpeechCaps::adopted_windows_methods().with_providers(&[
            (
                "whisper",
                dettivo_speech::engines::meeting_capable("whisper"),
            ),
            (
                "parakeet",
                dettivo_speech::engines::meeting_capable("parakeet"),
            ),
        ]),
        config: ConfigCaps::linux_methods(),
        insert: InsertCaps::linux_methods(),
        history: HistoryCaps::linux(),
        polish: PolishCaps::new(
            &["raw", "deterministic_polish", "enhanced"],
            &Preset::ALL.map(Preset::as_str),
            &Style::ALL.map(Style::as_str),
        ),
        llm: LlmCaps::linux(local_llm_available),
        hotkeys,
        meetings: MeetingCaps::linux(),
        rest,
    }
}

/// The compositor the session environment names: Hyprland by its instance
/// signature, otherwise the first entry of `XDG_CURRENT_DESKTOP`; `None`
/// when the environment says nothing.
pub fn compositor(env: &impl Fn(&str) -> Option<String>) -> Option<String> {
    if env("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        Some("Hyprland".to_string())
    } else {
        env("XDG_CURRENT_DESKTOP")
            .filter(|s| !s.is_empty())
            .map(|s| s.split(':').next().unwrap_or(&s).to_string())
    }
}

/// The platform block from the session environment, the chain's current
/// choice of insertion backend and the supervisor's tier.
pub fn platform(
    env: impl Fn(&str) -> Option<String>,
    insertion_backend: Option<String>,
    tier: &TierReport,
) -> PlatformCaps {
    let session = match env("XDG_SESSION_TYPE").as_deref() {
        Some("x11") => SessionKind::X11,
        Some("wayland") => SessionKind::Wayland,
        _ if env("WAYLAND_DISPLAY").is_some() => SessionKind::Wayland,
        _ if env("DISPLAY").is_some() => SessionKind::X11,
        _ => SessionKind::Wayland,
    };
    let compositor = compositor(&env);
    let insertion_backend = insertion_backend.unwrap_or_else(|| "none".to_string());
    PlatformCaps {
        os: "linux".to_string(),
        transport: "unix_socket".to_string(),
        compositor,
        session,
        insertion_backend,
        gpu: detect_gpu(),
        tier: tier.tier,
        tier_reason: tier.reason.clone(),
    }
}

/// `vulkan` when an ICD is installed; the engines run on Vulkan (ADR 0004).
fn detect_gpu() -> Option<String> {
    let has_icd = |dir: &str| {
        std::fs::read_dir(Path::new(dir))
            .map(|mut entries| entries.any(|e| e.is_ok()))
            .unwrap_or(false)
    };
    if has_icd("/usr/share/vulkan/icd.d") || has_icd("/etc/vulkan/icd.d") {
        Some("vulkan".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::capabilities::Tier;

    fn cpu_tier() -> TierReport {
        dettivo_speech::tier::detect(true, &[])
    }

    #[test]
    fn hyprland_wayland_session_is_detected() {
        let env = |k: &str| match k {
            "XDG_SESSION_TYPE" => Some("wayland".to_string()),
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("abc".to_string()),
            _ => None,
        };
        let p = platform(env, Some("virtual_keyboard".into()), &cpu_tier());
        assert_eq!(p.compositor.as_deref(), Some("Hyprland"));
        assert_eq!(p.tier, Tier::Cpu);
        assert!(p.tier_reason.contains("forces"));
        assert_eq!(p.session, SessionKind::Wayland);
        assert_eq!(p.insertion_backend, "virtual_keyboard");
        assert_eq!(p.os, "linux");
    }

    #[test]
    fn x11_session_reports_the_desktop_name_and_no_backend_when_none_is_available() {
        let env = |k: &str| match k {
            "XDG_SESSION_TYPE" => Some("x11".to_string()),
            "XDG_CURRENT_DESKTOP" => Some("GNOME:ubuntu".to_string()),
            _ => None,
        };
        let p = platform(env, None, &cpu_tier());
        assert_eq!(p.compositor.as_deref(), Some("GNOME"));
        assert_eq!(p.session, SessionKind::X11);
        assert_eq!(p.insertion_backend, "none");
    }

    #[test]
    fn snapshot_round_trips_through_the_contract_type() {
        let caps = capabilities(
            IpcMode::PeerToken,
            Some("xdotool".into()),
            HotkeyCaps {
                backend: "none".into(),
                available: Vec::new(),
            },
            true,
            RestCaps {
                enabled: true,
                port: 45_831,
                bind: "127.0.0.1".into(),
            },
            &cpu_tier(),
        );
        let json = serde_json::to_value(&caps).unwrap();
        let back: Capabilities = serde_json::from_value(json).unwrap();
        assert_eq!(back, caps);
        assert_eq!(back.auth.ipc_mode, IpcMode::PeerToken);
        assert!(back.rest.enabled);
        assert_eq!(back.rest.port, 45_831);
        assert_eq!(back.config.methods.len(), 7);
        assert_eq!(
            back.insert.methods,
            ["insert.undo", "insert.target", "insert.allow_self_target"]
        );
        assert_eq!(back.platform.insertion_backend, "xdotool");
        assert_eq!(back.hotkeys.backend, "none");
        assert!(back.speech.providers.contains_key("parakeet"));
        assert!(back.llm.local_available);
        assert!(
            back.llm
                .methods
                .contains(&"llm.models.download".to_string())
        );
    }
}
