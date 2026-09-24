//! `[polish]`: the macOS `PolishRulesConfig` schema version 1 serialised
//! to TOML — the transforms, the default preset and style, the named
//! custom rules, the app profiles and the custom presets. The vocabulary
//! is the macOS one, so a `polish.*` call means the same thing on both
//! ports. `[llm]` next door in [`super::llm_schema`] carries the provider
//! layer the Enhanced mode runs against, and is re-exported here so a
//! caller reads both sections from one place.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use super::llm_schema::{Llm, LlmProviderChoice};

/// A global transform the deterministic pass and the model prompt honour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Transform {
    /// Grammar and spelling slips (`ithink` to `I think`).
    #[serde(rename = "fixGrammar")]
    FixGrammar,
    /// Filler words (`uh`, `um`, `äh`).
    #[serde(rename = "removeFillers")]
    RemoveFillers,
    /// Capitalisation and terminal punctuation; never for the `code` preset.
    #[serde(rename = "smartPunctuation")]
    SmartPunctuation,
}

impl Transform {
    /// Every transform, in the macOS declaration order.
    pub const ALL: [Self; 3] = [
        Self::FixGrammar,
        Self::RemoveFillers,
        Self::SmartPunctuation,
    ];

    /// The wire and file spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixGrammar => "fixGrammar",
            Self::RemoveFillers => "removeFillers",
            Self::SmartPunctuation => "smartPunctuation",
        }
    }

    /// The name a settings screen shows.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::FixGrammar => "Fix Grammar & Spelling",
            Self::RemoveFillers => "Remove Filler Words",
            Self::SmartPunctuation => "Smart Punctuation",
        }
    }

    /// Whether the transform may run under `preset`.
    pub fn is_applicable(self, preset: Preset) -> bool {
        !matches!((self, preset), (Self::SmartPunctuation, Preset::Code))
    }
}

/// The target domain a rewrite is shaped for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// Email: paragraphs, greeting and signature kept.
    Email,
    /// Code and terminals: whitespace and tokens preserved, `@path` insertion.
    Code,
    /// Chat: short and conversational.
    Chat,
    /// Note-taking apps: bullets, numbered lists and headings kept.
    Notes,
    /// Everything else.
    Generic,
}

impl Preset {
    /// Every preset, in the macOS declaration order.
    pub const ALL: [Self; 5] = [
        Self::Email,
        Self::Code,
        Self::Chat,
        Self::Notes,
        Self::Generic,
    ];

    /// The wire and file spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Code => "code",
            Self::Chat => "chat",
            Self::Notes => "notes",
            Self::Generic => "generic",
        }
    }

    /// Parses the wire spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }

    /// The name `polish.presets.list` reports.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Email => "Email",
            Self::Code => "Code",
            Self::Chat => "Chat",
            Self::Notes => "Notes",
            Self::Generic => "General",
        }
    }

    /// The style a preset takes when the global style is `asDictated`.
    pub fn default_style(self) -> Style {
        match self {
            Self::Email => Style::Formal,
            Self::Code | Self::Chat | Self::Notes | Self::Generic => Style::AsDictated,
        }
    }

    /// The post-processors a preset runs by default.
    pub fn default_post_processors(self) -> Vec<PostProcessor> {
        match self {
            Self::Code => vec![PostProcessor::AtPrefixFilePaths],
            _ => Vec::new(),
        }
    }

    /// The valid spellings, for an error message.
    pub fn names() -> String {
        Self::ALL
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The tone register of a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Style {
    /// Cleanup only; the speaker's tone stays.
    #[serde(rename = "asDictated")]
    AsDictated,
    /// Capitals, full punctuation, business language.
    #[serde(rename = "formal")]
    Formal,
    /// Capitals, lighter punctuation.
    #[serde(rename = "casual")]
    Casual,
    /// Lowercase, minimal punctuation.
    #[serde(rename = "veryCasual")]
    VeryCasual,
}

impl Style {
    /// Every style, in the macOS declaration order.
    pub const ALL: [Self; 4] = [
        Self::AsDictated,
        Self::Formal,
        Self::Casual,
        Self::VeryCasual,
    ];

    /// The wire and file spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AsDictated => "asDictated",
            Self::Formal => "formal",
            Self::Casual => "casual",
            Self::VeryCasual => "veryCasual",
        }
    }

    /// Parses the wire spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// The name a settings screen shows.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::AsDictated => "As Dictated",
            Self::Formal => "Formal",
            Self::Casual => "Casual",
            Self::VeryCasual => "Very Casual",
        }
    }

    /// The instruction the prompt carries; empty for `asDictated`.
    pub fn instruction(self) -> &'static str {
        match self {
            Self::AsDictated => "",
            Self::Formal => "Use formal business language with proper capitalization.",
            Self::Casual => "Use casual tone with normal capitalization.",
            Self::VeryCasual => "Use very casual tone, lowercase ok, minimal punctuation.",
        }
    }

    /// The valid spellings, for an error message.
    pub fn names() -> String {
        Self::ALL
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A deterministic post-processor a preset or profile runs after the
/// transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PostProcessor {
    /// `foo.swift` becomes `@foo.swift` for code assistants.
    #[serde(rename = "atPrefixFilePaths")]
    AtPrefixFilePaths,
    /// `quality slash speed` becomes `quality/speed`.
    #[serde(rename = "normalizeSimpleSpokenSlash")]
    NormalizeSimpleSpokenSlash,
    /// `R and D` becomes `R&D`.
    #[serde(rename = "normalizeBusinessAbbreviations")]
    NormalizeBusinessAbbreviations,
    /// `thousand plus page` becomes `1,000-plus-page`.
    #[serde(rename = "normalizePlusPagePhrases")]
    NormalizePlusPagePhrases,
    /// Spoken bullets and ordinals become real lists.
    #[serde(rename = "normalizeSpokenLists")]
    NormalizeSpokenLists,
}

impl PostProcessor {
    /// Every processor, in the macOS declaration order (the order they run).
    pub const ALL: [Self; 5] = [
        Self::AtPrefixFilePaths,
        Self::NormalizeSimpleSpokenSlash,
        Self::NormalizeBusinessAbbreviations,
        Self::NormalizePlusPagePhrases,
        Self::NormalizeSpokenLists,
    ];

    /// The wire and file spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AtPrefixFilePaths => "atPrefixFilePaths",
            Self::NormalizeSimpleSpokenSlash => "normalizeSimpleSpokenSlash",
            Self::NormalizeBusinessAbbreviations => "normalizeBusinessAbbreviations",
            Self::NormalizePlusPagePhrases => "normalizePlusPagePhrases",
            Self::NormalizeSpokenLists => "normalizeSpokenLists",
        }
    }
}

/// One named custom rule (`polish.rules.*`); its content joins the
/// prompt's additional rules while it is enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolishRule {
    /// A lowercase UUID; `polish.rules.create` assigns it.
    pub id: String,
    /// The display name, what `polish.test { rules }` refers to.
    pub name: String,
    /// Whether the rule joins every rewrite.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// The instruction, at most [`CUSTOM_RULE_MAX_CHARS`] characters.
    pub content: String,
}

fn enabled_default() -> bool {
    true
}

/// The longest custom rule, in characters (the macOS limit).
pub const CUSTOM_RULE_MAX_CHARS: usize = 500;

/// The rule's content, or the message `polish.rules.create` and
/// `polish.rules.update` answer `INVALID_PARAMS` with: it names the limit
/// and what was sent.
pub fn validate_rule_content(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("a custom rule needs content".into());
    }
    let length = trimmed.chars().count();
    if length > CUSTOM_RULE_MAX_CHARS {
        return Err(format!(
            "a custom rule is at most {CUSTOM_RULE_MAX_CHARS} characters; this one is {length}"
        ));
    }
    Ok(trimmed.to_string())
}

/// One app profile: the preset an app id (or `prefix*`) takes, with an
/// optional style, post-processor list and custom preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEntry {
    /// The preset.
    pub preset: Preset,
    /// A style override; absent inherits the global style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// The post-processors; absent means the preset's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_processors: Option<Vec<PostProcessor>>,
    /// A custom preset name from `[polish] presets`; absent means the
    /// built-in preset above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_preset: Option<String>,
}

/// One custom preset: a built-in preset with its own style, transforms,
/// rules, post-processors and model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetEntry {
    /// The built-in preset it builds on.
    pub base: Preset,
    /// The style.
    pub style: Style,
    /// The transforms; absent means the global list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transforms: Option<Vec<Transform>>,
    /// Additional rules, at most 500 characters.
    #[serde(default)]
    pub custom_rules: String,
    /// The post-processors; absent means the base preset's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_processors: Option<Vec<PostProcessor>>,
    /// A model override for the Enhanced pass; absent means `[llm]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `[polish]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Polish {
    /// The enabled global transforms.
    pub transforms: Vec<Transform>,
    /// The preset an unmapped app takes.
    pub default_preset: Preset,
    /// The global style; `asDictated` lets each preset pick its own.
    pub default_style: Style,
    /// The named custom rules.
    pub rules: Vec<PolishRule>,
    /// App profiles keyed by the Wayland app id or X11 class (`prefix*`
    /// matches a family).
    pub apps: BTreeMap<String, AppEntry>,
    /// Custom presets keyed by name.
    pub presets: BTreeMap<String, PresetEntry>,
}

impl Default for Polish {
    fn default() -> Self {
        let mut apps = BTreeMap::new();
        apps.insert(
            "org.mozilla.Thunderbird".to_string(),
            AppEntry {
                preset: Preset::Email,
                style: None,
                post_processors: None,
                custom_preset: None,
            },
        );
        Self {
            transforms: Transform::ALL.to_vec(),
            default_preset: Preset::Generic,
            default_style: Style::AsDictated,
            rules: Vec::new(),
            apps,
            presets: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_smart_punctuation_is_gated_for_code() {
        for p in Preset::ALL {
            assert_eq!(Preset::parse(p.as_str()), Some(p));
            assert_eq!(
                serde_json::to_value(p).unwrap(),
                serde_json::Value::String(p.as_str().into())
            );
        }
        for s in Style::ALL {
            assert_eq!(Style::parse(s.as_str()), Some(s));
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                serde_json::Value::String(s.as_str().into())
            );
        }
        for t in Transform::ALL {
            assert_eq!(
                serde_json::to_value(t).unwrap(),
                serde_json::Value::String(t.as_str().into())
            );
        }
        assert!(!Transform::SmartPunctuation.is_applicable(Preset::Code));
        assert!(Transform::FixGrammar.is_applicable(Preset::Code));
        assert_eq!(Preset::Email.default_style(), Style::Formal);
        assert_eq!(
            Preset::Code.default_post_processors(),
            [PostProcessor::AtPrefixFilePaths]
        );
        assert_eq!(Preset::names(), "email, code, chat, notes, generic");
    }

    #[test]
    fn the_default_polish_section_maps_thunderbird_to_email() {
        let polish = Polish::default();
        assert_eq!(polish.apps["org.mozilla.Thunderbird"].preset, Preset::Email);
        assert_eq!(polish.transforms, Transform::ALL);
        let llm = Llm::default();
        assert_eq!(llm.timeout_ms, 8000);
        assert_eq!(llm.provider.as_str(), "auto");
    }

    #[test]
    fn an_over_long_custom_rule_is_rejected_naming_the_limit() {
        assert_eq!(
            validate_rule_content("  Keep it short.  "),
            Ok("Keep it short.".to_string())
        );
        assert_eq!(
            validate_rule_content("   "),
            Err("a custom rule needs content".to_string())
        );
        let long = "x".repeat(CUSTOM_RULE_MAX_CHARS + 1);
        assert_eq!(
            validate_rule_content(&long),
            Err("a custom rule is at most 500 characters; this one is 501".to_string())
        );
    }
}
