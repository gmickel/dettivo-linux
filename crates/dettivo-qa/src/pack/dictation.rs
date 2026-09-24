//! The dictation pack (ADR 0017): the dictation slice in order. The
//! release gate that used to extend it lives in `release` (ADR 0041).

use super::{Expect, Pack, Step, StepKind, scenario};

/// The dictation pack.
pub fn packs() -> Vec<Pack> {
    vec![Pack {
        name: "dictation",
        summary: "the dictation vertical slice: hotkeys, the pill, insertion, history and the first NFR numbers",
        steps: dictation_steps(),
    }]
}

fn dictation_steps() -> Vec<Step> {
    vec![
        scenario(
            "hotkeys_hyprland",
            Expect::SkipAllowed,
            "hotkey transitions",
        ),
        Step {
            id: "osd_dictation",
            kind: StepKind::Scenario,
            driver: Some("atspi"),
            expect: Expect::SkipAllowed,
            surface: None,
            contributes: "pill states",
        },
        Step {
            id: "osd_dictation",
            kind: StepKind::Scenario,
            driver: Some("cua"),
            expect: Expect::SkipAllowed,
            surface: None,
            contributes: "pill states",
        },
        scenario(
            "insertion_matrix",
            Expect::SkipAllowed,
            "insertion_reliability (NFR-3)",
        ),
        scenario(
            "never_into_self",
            Expect::SkipAllowed,
            "self-insertion refusal",
        ),
        scenario("history_roundtrip", Expect::Pass, "history round trip"),
        scenario(
            "first_insert_timing",
            Expect::SkipAllowed,
            "first_insert p50/p95 (NFR-1)",
        ),
        Step {
            id: "whisper_wer",
            kind: StepKind::WhisperWer,
            driver: None,
            expect: Expect::Pass,
            surface: None,
            contributes: "wer",
        },
    ]
}
