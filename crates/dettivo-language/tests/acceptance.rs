//! Acceptance through `pipeline::run` (the order dictation runs: the raw
//! layer first, then the deterministic Polish pass), where the unit
//! goldens feed the later stages the spoken phrase directly. Each case
//! asserts the string that would be inserted for a representative prose,
//! command, file-name and model-name dictation with spoken punctuation
//! on, so the raw layer's `dash` and `colon` marks and the later stages'
//! technical-syntax recognisers are proven to agree on one path.

use dettivo_core::config::polish_schema::{Llm, Polish};
use dettivo_language::pipeline::{Mode, Settings, run};
use dettivo_language::policy::RulesConfig;

fn settings<'a>(rules: &'a RulesConfig, llm: &'a Llm) -> Settings<'a> {
    Settings {
        replacements: &[],
        spoken_punctuation: true,
        protect_tokens: true,
        vocabulary: &[],
        rules,
        llm,
        local: None,
    }
}

/// The inserted text for `input` through the whole path, with the preset
/// the application context selects (a terminal or IDE picks Code).
fn inserted(input: &str, app_id: Option<&str>) -> String {
    let rules = RulesConfig::from_config(&Polish::default());
    let llm = Llm::default();
    let settings = settings(&rules, &llm);
    run(Mode::DeterministicPolish, input, app_id, &settings).text
}

#[test]
fn spoken_technical_syntax_survives_the_raw_layer_into_the_polish_pass() {
    let cases = [
        // Prose: the spoken marks become punctuation, the filler goes.
        (
            "um so the meeting moved to thursday comma which is fine period",
            None,
            "So the meeting moved to thursday, which is fine.",
        ),
        // A model tag: the fixed pass reads `colon` before the raw layer
        // would have turned it into a mark, in prose and in code alike.
        (
            "run ollama pull qwen three colon 4b",
            None,
            "Run ollama pull qwen3:4b.",
        ),
        (
            "run ollama pull qwen three colon 4b",
            Some("com.mitchellh.ghostty"),
            "Run ollama pull qwen3:4b",
        ),
        // A flag and a file name in a terminal: the doubled dash and the
        // spoken dot reach the code preset's technical pass whole.
        (
            "keep settings view dot swift and flag dash dash model qwen three colon 4b",
            Some("com.mitchellh.ghostty"),
            "Keep settings @view.swift and flag -- model qwen3:4b",
        ),
        // The same words in prose stay words: a doubled dash is never two
        // marks, and no code pass runs.
        (
            "keep settings view dot swift and flag dash dash model qwen three colon 4b",
            None,
            "Keep settings view dot swift and flag dash dash model qwen3:4b.",
        ),
        // A single spoken dash is still the mark.
        ("wait dash then go", None, "Wait - then go."),
    ];
    let mut wrong = Vec::new();
    for (input, app_id, want) in cases {
        let got = inserted(input, app_id);
        eprintln!("PROBE {input:?} {app_id:?} -> {got:?}");
        if got != want {
            wrong.push(format!("{input:?} {app_id:?}: got {got:?}, want {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
