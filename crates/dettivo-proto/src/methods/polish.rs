//! `polish.*`: rules, presets, test and app mapping
//! (`docs/api/dettivo-ipc-v1.md` section 8.6).

use crate::id::Id;
use serde::{Deserialize, Serialize};

/// A user-defined polish rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// The rule's id.
    pub rule_id: Id,
    /// Display name.
    pub name: String,
    /// Whether the rule is applied.
    pub enabled: bool,
    /// Rule instruction content.
    pub content: String,
}

/// `polish.rules.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesListResult {
    /// All configured rules.
    pub rules: Vec<Rule>,
}

/// `polish.rules.create` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesCreateParams {
    /// Display name.
    pub name: String,
    /// Whether the rule starts enabled.
    pub enabled: bool,
    /// Rule instruction content.
    pub content: String,
}

/// `polish.rules.create` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesCreateResult {
    /// The new rule's id.
    pub rule_id: Id,
}

/// `polish.rules.update` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesUpdateParams {
    /// The rule to update.
    pub rule_id: Id,
    /// New enabled state.
    pub enabled: bool,
    /// New instruction content.
    pub content: String,
}

/// `polish.rules.update` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesUpdateResult {
    /// The updated rule's id.
    pub rule_id: Id,
    /// Always `true` on success.
    pub updated: bool,
}

/// `polish.rules.delete` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesDeleteParams {
    /// The rule to delete.
    pub rule_id: Id,
}

/// `polish.rules.delete` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesDeleteResult {
    /// Always `true` on success.
    pub deleted: bool,
}

/// One built-in polish preset, as the macOS server lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    /// Preset id, the value `polish.test` and `polish.apps.set` accept.
    pub id: String,
    /// Display name.
    pub display_name: String,
}

/// `polish.presets.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetsListResult {
    /// Available presets.
    pub presets: Vec<Preset>,
}

/// `polish.test` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestParams {
    /// Sample input text.
    pub input: String,
    /// Preset id to apply.
    pub preset: String,
    /// Named rules to apply in addition to the preset.
    pub rules: Vec<String>,
    /// Linux addition: the style id; absent takes the preset's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Linux addition: the app id the policy resolves for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// Linux addition: `raw`, `deterministic_polish` (the default) or
    /// `enhanced`; only `enhanced` reaches the provider layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// `polish.test` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestResult {
    /// Polished output text: what the requested mode would insert.
    pub output: String,
    /// The named rules that were applied.
    pub applied_rules: Vec<String>,
    /// The model that performed the polish, or `deterministic`.
    pub model: String,
    /// Linux addition: the text after the raw layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    /// Linux addition: the text after the deterministic Polish pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polished: Option<String>,
    /// Linux addition: the model rewrite, when `mode` was `enhanced` and
    /// one was inserted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enhanced: Option<String>,
    /// Linux addition: why the Polish result was used instead of a
    /// rewrite (`provider_unavailable`, `guard_rejected`,
    /// `fallback_used`), with the reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<PolishNotice>,
    /// Linux addition: the hash of the policy this ran under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
}

/// Why an Enhanced pass inserted the deterministic Polish result (Linux
/// addition; it rides on `polish.test`, the completion event and the
/// history item).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolishNotice {
    /// `provider_unavailable`, `guard_rejected` or `fallback_used`.
    pub kind: String,
    /// One line for a person.
    pub reason: String,
}

/// An app-to-preset mapping: one row of `polish.apps.list`, and both the
/// params and the result of `polish.apps.set`, which echoes what it set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppMapping {
    /// The app's bundle or application identifier.
    pub bundle_id: String,
    /// The preset id assigned to that app.
    pub preset: String,
}

/// `polish.apps.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppsListResult {
    /// Configured mappings.
    pub mappings: Vec<AppMapping>,
}
