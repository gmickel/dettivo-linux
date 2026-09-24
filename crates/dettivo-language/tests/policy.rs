//! The resolution order and the frozen hash (R1): a session's policy
//! comes from the user's app profile first, then the built-in mapping,
//! then the app class, then the global defaults, and the hash it carries
//! changes with every input to that resolution and with nothing else.

use std::collections::BTreeSet;

use dettivo_core::config::polish_schema::{AppEntry, Polish, PolishRule, PresetEntry};
use dettivo_language::policy::{
    AppClass, AppProfile, Backend, CUSTOM_RULES_MAX_CHARS, Context, RulesConfig, default_mapping,
    resolve,
};
use dettivo_language::polish::{PostProcessor, Preset, Style, Transform};

fn backend() -> Backend {
    Backend::Ollama {
        endpoint: "http://localhost:11434".into(),
        model: "qwen3:4b-instruct".into(),
    }
}

fn config() -> RulesConfig {
    RulesConfig::from_config(&Polish::default())
}

fn app(id: &str) -> Context<'static> {
    Context {
        app_id: Some(Box::leak(id.to_string().into_boxed_str())),
        ..Context::default()
    }
}

#[test]
fn a_user_app_profile_wins_over_the_built_in_mapping() {
    let mut polish = Polish::default();
    polish.apps.insert(
        "code".into(),
        AppEntry {
            preset: Preset::Notes,
            style: Some(Style::Casual),
            post_processors: None,
            custom_preset: None,
        },
    );
    let config = RulesConfig::from_config(&polish);
    // The built-in mapping would make an editor `code`.
    assert_eq!(default_mapping("code").map(|(p, _)| p), Some(Preset::Code));
    let policy = resolve(&config, &backend(), &app("code"));
    assert_eq!(policy.preset, Preset::Notes);
    assert_eq!(policy.style, Style::Casual);
}

#[test]
fn the_built_in_mapping_answers_before_the_class_and_the_global_default() {
    let config = config();
    let mail = resolve(&config, &backend(), &app("org.mozilla.Thunderbird"));
    assert_eq!(mail.preset, Preset::Email);
    assert_eq!(mail.style, Style::Formal);
    // Smart punctuation is not applicable to the code preset, so the
    // editor mapping drops it from the frozen set.
    let editor = resolve(&config, &backend(), &app("zed"));
    assert_eq!(editor.preset, Preset::Code);
    assert!(!editor.transforms.contains(&Transform::SmartPunctuation));
    assert_eq!(
        editor.post_processors,
        vec![PostProcessor::AtPrefixFilePaths]
    );
    let by_class = resolve(
        &config,
        &backend(),
        &Context {
            app_id: Some("com.example.unknown"),
            app_class: Some(AppClass::Mail),
            raw_transcript: None,
        },
    );
    assert_eq!(by_class.preset, Preset::Email);
    assert_eq!(by_class.style, Style::Formal);
    let global = resolve(&config, &backend(), &app("com.example.unknown"));
    assert_eq!(global.preset, Preset::Generic);
}

#[test]
fn a_terminal_gets_the_generic_preset_for_prose_and_the_code_preset_for_commands() {
    let config = config();
    let prose = resolve(
        &config,
        &backend(),
        &Context {
            app_id: Some("Alacritty"),
            app_class: None,
            raw_transcript: Some("i think we should review this before we ship it tomorrow"),
        },
    );
    assert_eq!(prose.preset, Preset::Generic);
    let command = resolve(
        &config,
        &backend(),
        &Context {
            app_id: Some("Alacritty"),
            app_class: None,
            raw_transcript: Some("cargo test --workspace"),
        },
    );
    assert_eq!(command.preset, Preset::Code);
}

#[test]
fn a_custom_preset_carries_its_style_transforms_rules_and_model() {
    let mut polish = Polish::default();
    polish.presets.insert(
        "legal".into(),
        PresetEntry {
            base: Preset::Email,
            style: Style::Formal,
            transforms: Some(vec![Transform::FixGrammar]),
            custom_rules: "Never abbreviate a party name.".into(),
            post_processors: Some(vec![PostProcessor::NormalizeBusinessAbbreviations]),
            model: Some("qwen3:8b".into()),
        },
    );
    polish.apps.insert(
        "org.kde.kmail2".into(),
        AppEntry {
            preset: Preset::Email,
            style: None,
            post_processors: None,
            custom_preset: Some("legal".into()),
        },
    );
    polish.rules.push(PolishRule {
        id: "r1".into(),
        name: "House style".into(),
        enabled: true,
        content: "Keep sentences short.".into(),
    });
    let config = RulesConfig::from_config(&polish);
    let policy = resolve(&config, &backend(), &app("org.kde.kmail2"));
    assert_eq!(policy.preset, Preset::Email);
    assert_eq!(policy.style, Style::Formal);
    assert_eq!(policy.transforms, BTreeSet::from([Transform::FixGrammar]));
    assert_eq!(
        policy.custom_rules,
        "Keep sentences short.\nNever abbreviate a party name."
    );
    assert_eq!(policy.backend.model_name(), "qwen3:8b");
    assert!(
        policy
            .post_processors
            .contains(&PostProcessor::NormalizeBusinessAbbreviations)
    );
    // A profile naming a custom preset that does not exist falls back to
    // its built-in preset rather than failing the session.
    let mut orphan = polish.clone();
    orphan.presets.clear();
    let config = RulesConfig::from_config(&orphan);
    assert_eq!(
        resolve(&config, &backend(), &app("org.kde.kmail2")).preset,
        Preset::Email
    );
}

#[test]
fn a_disabled_rule_never_reaches_the_prompt_and_a_long_one_is_clipped() {
    let mut polish = Polish::default();
    polish.rules.push(PolishRule {
        id: "off".into(),
        name: "Off".into(),
        enabled: false,
        content: "Never used.".into(),
    });
    polish.rules.push(PolishRule {
        id: "long".into(),
        name: "Long".into(),
        enabled: true,
        content: "x".repeat(CUSTOM_RULES_MAX_CHARS + 100),
    });
    let config = RulesConfig::from_config(&polish);
    assert!(!config.custom_rules.contains("Never used."));
    assert_eq!(config.custom_rules.chars().count(), CUSTOM_RULES_MAX_CHARS);
}

#[test]
fn the_hash_is_stable_and_changes_with_every_input_to_the_resolution() {
    let config = config();
    let policy = resolve(&config, &backend(), &app("com.example.unknown"));
    let hash = policy.config_hash();
    assert_eq!(hash.len(), 8);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(hash, policy.config_hash(), "the hash is deterministic");
    assert_eq!(
        hash,
        resolve(&config, &backend(), &app("com.example.unknown")).config_hash()
    );
    let other_app = resolve(&config, &backend(), &app("org.mozilla.Thunderbird")).config_hash();
    assert_ne!(hash, other_app);
    let other_backend = resolve(
        &config,
        &Backend::Mock {
            model: "mock-echo".into(),
        },
        &app("com.example.unknown"),
    )
    .config_hash();
    assert_ne!(hash, other_backend);
    let mut with_rule = Polish::default();
    with_rule.rules.push(PolishRule {
        id: "r".into(),
        name: "R".into(),
        enabled: true,
        content: "Keep it short.".into(),
    });
    let with_rule = RulesConfig::from_config(&with_rule);
    assert_ne!(
        hash,
        resolve(&with_rule, &backend(), &app("com.example.unknown")).config_hash()
    );
}

#[test]
fn a_wildcard_profile_matches_a_family_longest_prefix_first() {
    let candidates = vec![
        AppProfile::new("jetbrains-*", Preset::Code, None),
        AppProfile::new("jetbrains-idea*", Preset::Notes, None),
        AppProfile::new("zed", Preset::Chat, None),
    ];
    assert_eq!(
        AppProfile::matches("jetbrains-idea-ce", &candidates).map(|p| p.preset),
        Some(Preset::Notes)
    );
    assert_eq!(
        AppProfile::matches("jetbrains-clion", &candidates).map(|p| p.preset),
        Some(Preset::Code)
    );
    assert_eq!(
        AppProfile::matches("zed", &candidates).map(|p| p.preset),
        Some(Preset::Chat)
    );
    assert!(AppProfile::matches("firefox", &candidates).is_none());
}
