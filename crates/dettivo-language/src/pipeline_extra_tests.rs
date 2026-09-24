use super::*;
use crate::enhanced::NoticeKind;
use dettivo_core::config::polish_schema::Polish;

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

#[test]
fn enhanced_without_a_provider_inserts_the_polish_text_with_a_notice() {
    let rules = RulesConfig::from_config(&Polish::default());
    let llm = Llm {
        provider: dettivo_core::config::polish_schema::LlmProviderChoice::Local,
        ..Llm::default()
    };
    let out = run(
        Mode::Enhanced,
        "um so we should rewrite the onboarding flow before the release lands",
        None,
        &settings(&rules, &llm),
    );
    assert_eq!(out.text, out.polished);
    assert_eq!(
        out.notice.map(|n| n.kind),
        Some(NoticeKind::ProviderUnavailable)
    );
    assert_eq!(out.model, "deterministic");
}

#[test]
fn the_mode_identifiers_are_the_macos_ones() {
    assert_eq!(Mode::parse("polish"), Some(Mode::DeterministicPolish));
    assert_eq!(
        Mode::parse("deterministic_polish"),
        Some(Mode::DeterministicPolish)
    );
    assert_eq!(Mode::parse("enhanced"), Some(Mode::Enhanced));
    assert_eq!(Mode::parse("raw"), Some(Mode::Raw));
    assert_eq!(Mode::parse("meeting"), None);
    assert_eq!(Mode::DeterministicPolish.as_str(), "deterministic_polish");
}
