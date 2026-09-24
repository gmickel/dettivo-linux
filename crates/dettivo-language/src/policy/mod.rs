//! The effective Polish policy (FR-M4, FR-M5): the macOS
//! `PolishRulesConfig` schema version 1 as the daemon reads it from
//! `[polish]`, the resolution a session freezes at start (user app
//! profile, then the built-in app mapping, then the app class, then the
//! global defaults, with the terminal heuristics that route prose typed
//! into a terminal to the generic preset), and the deterministic hash the
//! logs and the history item carry.

mod effective;
mod heuristics;

use std::collections::{BTreeMap, BTreeSet};

use dettivo_core::config::polish_schema::{AppEntry, Polish, PresetEntry};
use serde::{Deserialize, Serialize};

pub use effective::{AppClass, Backend, EffectivePolicy};

use crate::polish::{PostProcessor, Preset, Style, Transform};
use heuristics::{
    dynamic_preset_override, is_terminal_app, transcript_likely_contains_recoverable_path,
};

/// The app id the macOS Codex profile carries; a client that passes it
/// gets the same post-processors on Linux.
pub const CODEX_APP_ID: &str = "com.openai.codex";

/// One app profile: an app id (or `prefix*`) and what it overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProfile {
    /// The Wayland app id or X11 class, `prefix*` for a family.
    pub app_id: String,
    /// The preset.
    pub preset: Preset,
    /// A style override; `None` inherits the global style.
    pub style_override: Option<Style>,
    /// The post-processors; `None` means the preset's defaults.
    pub post_processors: Option<Vec<PostProcessor>>,
    /// A custom preset name.
    pub custom_preset: Option<String>,
}

impl AppProfile {
    /// A profile with the preset and style only.
    pub fn new(app_id: &str, preset: Preset, style: Option<Style>) -> Self {
        Self {
            app_id: app_id.to_string(),
            preset,
            style_override: style,
            post_processors: None,
            custom_preset: None,
        }
    }

    /// Exact match first, then the longest `prefix*`; ties break on the
    /// lexically smaller pattern.
    pub fn matches<'a>(app_id: &str, candidates: &'a [AppProfile]) -> Option<&'a AppProfile> {
        if let Some(exact) = candidates.iter().find(|p| p.app_id == app_id) {
            return Some(exact);
        }
        let mut wildcards: Vec<(&AppProfile, &str)> = candidates
            .iter()
            .filter_map(|p| {
                let prefix = p.app_id.strip_suffix('*')?;
                app_id.starts_with(prefix).then_some((p, prefix))
            })
            .collect();
        wildcards.sort_by(|a, b| {
            b.1.chars()
                .count()
                .cmp(&a.1.chars().count())
                .then_with(|| a.0.app_id.cmp(&b.0.app_id))
        });
        wildcards.first().map(|(p, _)| *p)
    }
}

/// A custom preset: a built-in preset with its own style, transforms,
/// rules, post-processors and model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomPreset {
    /// The name (the `[polish] presets` key).
    pub name: String,
    /// The built-in preset it builds on.
    pub base_preset: Preset,
    /// The style.
    pub style: Style,
    /// The transforms; `None` means the global list.
    pub transforms: Option<BTreeSet<Transform>>,
    /// Additional rules.
    pub custom_rules: String,
    /// The post-processors; `None` means the base preset's defaults.
    pub post_processors: Option<Vec<PostProcessor>>,
    /// A model override for the Enhanced pass.
    pub model_override: Option<String>,
}

/// The `PolishRulesConfig` schema version 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesConfig {
    /// The enabled global transforms.
    pub transforms: BTreeSet<Transform>,
    /// The preset an unmapped app takes.
    pub default_preset: Preset,
    /// The global style.
    pub default_style: Style,
    /// The global custom rules (the enabled named rules, one per line).
    pub custom_rules: String,
    /// App profiles keyed by app id.
    pub app_profiles: BTreeMap<String, AppProfile>,
    /// Custom presets keyed by name.
    pub custom_presets: BTreeMap<String, CustomPreset>,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            transforms: Transform::ALL.into_iter().collect(),
            default_preset: Preset::Generic,
            default_style: Style::AsDictated,
            custom_rules: String::new(),
            app_profiles: BTreeMap::new(),
            custom_presets: BTreeMap::new(),
        }
    }
}

/// The longest custom rule text, in characters.
pub const CUSTOM_RULES_MAX_CHARS: usize = 500;

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

impl RulesConfig {
    /// The config as `[polish]` states it: enabled rules join the global
    /// rules, profiles and presets are trimmed and validated the way
    /// macOS `validate()` does (empty ids dropped, rules clipped, a custom
    /// preset reference that does not exist removed).
    pub fn from_config(polish: &Polish) -> Self {
        let custom_rules = polish
            .rules
            .iter()
            .filter(|r| r.enabled)
            .map(|r| clip(r.content.trim(), CUSTOM_RULES_MAX_CHARS))
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let custom_presets: BTreeMap<String, CustomPreset> = polish
            .presets
            .iter()
            .filter_map(|(name, entry)| {
                let name = name.trim();
                (!name.is_empty()).then(|| (name.to_string(), custom_preset(name, entry)))
            })
            .collect();
        let app_profiles: BTreeMap<String, AppProfile> = polish
            .apps
            .iter()
            .filter_map(|(id, entry)| {
                let id = id.trim();
                (!id.is_empty()).then(|| (id.to_string(), app_profile(id, entry, &custom_presets)))
            })
            .collect();
        Self {
            transforms: polish.transforms.iter().copied().collect(),
            default_preset: polish.default_preset,
            default_style: polish.default_style,
            custom_rules,
            app_profiles,
            custom_presets,
        }
    }
}

fn custom_preset(name: &str, entry: &PresetEntry) -> CustomPreset {
    CustomPreset {
        name: name.to_string(),
        base_preset: entry.base,
        style: entry.style,
        transforms: entry
            .transforms
            .as_ref()
            .map(|t| t.iter().copied().collect()),
        custom_rules: clip(entry.custom_rules.trim(), CUSTOM_RULES_MAX_CHARS),
        post_processors: entry.post_processors.clone(),
        model_override: entry.model.clone().filter(|m| !m.trim().is_empty()),
    }
}

fn app_profile(id: &str, entry: &AppEntry, presets: &BTreeMap<String, CustomPreset>) -> AppProfile {
    AppProfile {
        app_id: id.to_string(),
        preset: entry.preset,
        style_override: entry.style,
        post_processors: entry.post_processors.clone(),
        custom_preset: entry
            .custom_preset
            .clone()
            .filter(|name| presets.contains_key(name)),
    }
}

/// What a session knows about its target when the policy is resolved.
#[derive(Debug, Clone, Default)]
pub struct Context<'a> {
    /// The focused app id.
    pub app_id: Option<&'a str>,
    /// The app class, when known.
    pub app_class: Option<AppClass>,
    /// The raw transcript, for the terminal heuristics.
    pub raw_transcript: Option<&'a str>,
}

/// The built-in app mapping: Linux app ids and window classes that map to
/// a preset (and a style) before the global defaults apply.
pub fn default_mapping(app_id: &str) -> Option<(Preset, Option<Style>)> {
    let mapping: Option<(Preset, Option<Style>)> = match app_id {
        "org.mozilla.Thunderbird"
        | "thunderbird"
        | "org.gnome.Evolution"
        | "org.kde.kmail2"
        | "kmail" => Some((Preset::Email, Some(Style::Formal))),
        "code"
        | "Code"
        | "code-oss"
        | "code-url-handler"
        | "codium"
        | "VSCodium"
        | "cursor"
        | "Cursor"
        | "zed"
        | "dev.zed.Zed"
        | "neovide"
        | "jetbrains-idea"
        | "jetbrains-clion"
        | "jetbrains-pycharm"
        | "jetbrains-rustrover" => Some((Preset::Code, None)),
        "com.openai.codex" => Some((Preset::Generic, None)),
        "Slack"
        | "slack"
        | "discord"
        | "Discord"
        | "org.telegram.desktop"
        | "Signal"
        | "signal"
        | "Element"
        | "element"
        | "org.mozilla.Thunderbird.chat" => Some((Preset::Chat, Some(Style::Casual))),
        "obsidian" | "Obsidian" | "Logseq" | "logseq" | "org.gnome.Notes" | "Zoom" | "zoom" => {
            Some((Preset::Notes, None))
        }
        _ if is_terminal_app(app_id) => Some((Preset::Code, None)),
        _ => None,
    };
    mapping
}

/// Resolves the effective policy in the macOS order.
pub fn resolve(config: &RulesConfig, backend: &Backend, ctx: &Context<'_>) -> EffectivePolicy {
    let app_id = ctx.app_id.map(str::trim).filter(|s| !s.is_empty());
    let user_profile = app_id.and_then(|id| {
        let candidates: Vec<AppProfile> = config.app_profiles.values().cloned().collect();
        AppProfile::matches(id, &candidates).cloned()
    });
    let default_profile = match (&user_profile, app_id) {
        (None, Some(id)) => default_mapping(id).map(|(preset, style)| AppProfile {
            app_id: id.to_string(),
            preset,
            style_override: style,
            post_processors: Some(preset.default_post_processors()),
            custom_preset: None,
        }),
        _ => None,
    };
    let profile = user_profile.clone().or(default_profile);
    let custom = profile
        .as_ref()
        .and_then(|p| p.custom_preset.as_ref())
        .and_then(|name| config.custom_presets.get(name));
    let explicit = user_profile.is_some() || custom.is_some();
    let dynamic = dynamic_preset_override(app_id, ctx.raw_transcript, explicit);
    let preset = dynamic
        .or(custom.map(|c| c.base_preset))
        .or(profile.as_ref().map(|p| p.preset))
        .or(preset_for_class(ctx.app_class))
        .unwrap_or(config.default_preset);
    let style = custom
        .map(|c| c.style)
        .or(profile.as_ref().and_then(|p| p.style_override))
        .or(style_for_class(ctx.app_class))
        .unwrap_or(if config.default_style == Style::AsDictated {
            preset.default_style()
        } else {
            config.default_style
        });
    let transforms: BTreeSet<Transform> = custom
        .and_then(|c| c.transforms.clone())
        .unwrap_or_else(|| config.transforms.clone())
        .into_iter()
        .filter(|t| t.is_applicable(preset))
        .collect();
    let backend = match custom.and_then(|c| c.model_override.as_deref()) {
        Some(model) => backend.with_model(model),
        None => backend.clone(),
    };
    let terminal_prose = dynamic == Some(Preset::Generic);
    let post_processors = resolve_post_processors(
        custom,
        profile.as_ref(),
        preset,
        app_id,
        terminal_prose,
        ctx.raw_transcript,
    );
    let custom_rules = merged_rules(
        &config.custom_rules,
        custom.map(|c| c.custom_rules.as_str()),
    );
    EffectivePolicy {
        preset,
        style,
        transforms,
        custom_rules,
        backend,
        post_processors,
        target_app_id: app_id.map(str::to_string),
        app_class: ctx.app_class,
    }
}

fn merged_rules(global: &str, custom: Option<&str>) -> String {
    let global = global.trim();
    let custom = custom.map(str::trim).unwrap_or("");
    match (global.is_empty(), custom.is_empty()) {
        (true, _) => custom.to_string(),
        (_, true) => global.to_string(),
        _ => format!("{global}\n{custom}"),
    }
}

fn preset_for_class(class: Option<AppClass>) -> Option<Preset> {
    match class {
        Some(AppClass::Ide) => Some(Preset::Code),
        Some(AppClass::Mail) => Some(Preset::Email),
        Some(AppClass::Browser) => Some(Preset::Chat),
        Some(AppClass::Generic) | None => None,
    }
}

fn style_for_class(class: Option<AppClass>) -> Option<Style> {
    match class {
        Some(AppClass::Mail) => Some(Style::Formal),
        _ => None,
    }
}

fn resolve_post_processors(
    custom: Option<&CustomPreset>,
    profile: Option<&AppProfile>,
    preset: Preset,
    app_id: Option<&str>,
    terminal_prose: bool,
    raw_transcript: Option<&str>,
) -> Vec<PostProcessor> {
    let base: Vec<PostProcessor> = if terminal_prose {
        preset.default_post_processors()
    } else {
        custom
            .and_then(|c| c.post_processors.clone())
            .or_else(|| profile.and_then(|p| p.post_processors.clone()))
            .unwrap_or_else(|| preset.default_post_processors())
    };
    let mut merged: BTreeSet<PostProcessor> = base.into_iter().collect();
    if preset != Preset::Code {
        merged.insert(PostProcessor::NormalizeSpokenLists);
    }
    if app_id == Some(CODEX_APP_ID) || terminal_prose {
        merged.insert(PostProcessor::NormalizeSimpleSpokenSlash);
        merged.insert(PostProcessor::NormalizeBusinessAbbreviations);
        merged.insert(PostProcessor::NormalizePlusPagePhrases);
    }
    if terminal_prose && transcript_likely_contains_recoverable_path(raw_transcript) {
        merged.insert(PostProcessor::AtPrefixFilePaths);
    }
    PostProcessor::ALL
        .into_iter()
        .filter(|p| merged.contains(p))
        .collect()
}
