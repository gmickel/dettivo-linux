//! `polish.*`: the custom rules, the presets, the app mapping and the
//! sample run (`docs/api/dettivo-ipc-v1.md` section 8.6, ADR 0023). Every
//! write goes through the comment-preserving config edit, so a `[polish]`
//! section a person wrote by hand keeps its comments and its ordering.

use dettivo_core::config::edit;
use dettivo_core::config::polish_schema::{
    AppEntry, PolishRule, Preset, Style, validate_rule_content,
};
use dettivo_language::pipeline::{self, Mode, Settings};
use dettivo_language::policy::RulesConfig;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::id::Id;
use dettivo_proto::methods::polish::{
    AppMapping, AppsListResult, PolishNotice, Preset as WirePreset, PresetsListResult, Rule,
    RulesCreateParams, RulesCreateResult, RulesDeleteParams, RulesDeleteResult, RulesListResult,
    RulesUpdateParams, RulesUpdateResult, TestParams, TestResult,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::handlers::config::edit_values;

fn invalid(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

fn preset_of(name: &str) -> Result<Preset, JsonRpcError> {
    Preset::parse(name.trim()).ok_or_else(|| {
        invalid(format!(
            "preset {name:?} is unknown; the presets are {}",
            Preset::names()
        ))
    })
}

fn style_of(name: &str) -> Result<Style, JsonRpcError> {
    Style::parse(name.trim()).ok_or_else(|| {
        invalid(format!(
            "style {name:?} is unknown; the styles are {}",
            Style::names()
        ))
    })
}

/// Writes `[polish] rules` back, comments kept.
/// One change to `[polish] rules`, derived from the rules the file holds
/// under the edit lock; `change` returns `None` when the rule it names is
/// not there.
fn edit_rules(
    daemon: &Daemon,
    change: impl FnOnce(&mut Vec<PolishRule>) -> Option<()>,
) -> Result<(), JsonRpcError> {
    edit_values(daemon, |text, current| {
        let mut rules = current.polish.rules;
        change(&mut rules).ok_or_else(|| {
            JsonRpcError::new(AppCode::NotFound, "Rule not found", ErrorDetails::empty())
        })?;
        let value = serde_json::to_value(&rules).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?;
        edit::set_structured(&text, "polish.rules", &value).map_err(|e| {
            JsonRpcError::new(AppCode::InvalidParams, e.message, ErrorDetails::empty())
        })
    })
}

/// The custom rule the QA seed writes (`DETTIVO_E2E_SEED=1`), so the
/// `polish.rules.*` fixtures replay against a daemon that has one.
pub const SEED_RULE_ID: &str = "c0a80101-0000-4000-8000-000000000001";

/// Writes the seed rule into `[polish] rules` when it is not there yet;
/// returns whether anything was written.
pub fn seed(daemon: &Daemon) -> Result<bool, JsonRpcError> {
    if daemon
        .config()
        .config
        .polish
        .rules
        .iter()
        .any(|r| r.id == SEED_RULE_ID)
    {
        return Ok(false);
    }
    let rule = PolishRule {
        id: SEED_RULE_ID.to_string(),
        name: "Professional Tone".into(),
        enabled: true,
        content: "Use concise professional phrasing".into(),
    };
    edit_rules(daemon, |rules| {
        if !rules.iter().any(|r| r.id == SEED_RULE_ID) {
            rules.push(rule);
        }
        Some(())
    })?;
    Ok(true)
}

/// `polish.rules.list`.
pub fn rules_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let loaded = daemon.config();
    let rules = loaded
        .config
        .polish
        .rules
        .iter()
        .map(|r| {
            Ok(Rule {
                rule_id: Id::new(r.id.clone()).map_err(|e| invalid(e.to_string()))?,
                name: r.name.clone(),
                enabled: r.enabled,
                content: r.content.clone(),
            })
        })
        .collect::<Result<Vec<_>, JsonRpcError>>()?;
    json(RulesListResult { rules })
}

/// `polish.rules.create`.
pub fn rules_create(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RulesCreateParams = params(params_value)?;
    let name = p.name.trim();
    if name.is_empty() {
        return Err(invalid("a custom rule needs a name"));
    }
    let content = validate_rule_content(&p.content).map_err(invalid)?;
    let id = uuid::Uuid::new_v4().to_string();
    let rule = PolishRule {
        id: id.clone(),
        name: name.to_string(),
        enabled: p.enabled,
        content,
    };
    edit_rules(daemon, |rules| {
        rules.push(rule);
        Some(())
    })?;
    json(RulesCreateResult {
        rule_id: Id::new(id).map_err(|e| invalid(e.to_string()))?,
    })
}

/// `polish.rules.update`.
pub fn rules_update(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RulesUpdateParams = params(params_value)?;
    let content = validate_rule_content(&p.content).map_err(invalid)?;
    let id = p.rule_id.as_str().to_string();
    edit_rules(daemon, |rules| {
        let rule = rules.iter_mut().find(|r| r.id == id)?;
        rule.enabled = p.enabled;
        rule.content = content;
        Some(())
    })?;
    json(RulesUpdateResult {
        rule_id: p.rule_id,
        updated: true,
    })
}

/// `polish.rules.delete`.
pub fn rules_delete(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RulesDeleteParams = params(params_value)?;
    let id = p.rule_id.as_str().to_string();
    edit_rules(daemon, |rules| {
        let before = rules.len();
        rules.retain(|r| r.id != id);
        (rules.len() < before).then_some(())
    })?;
    json(RulesDeleteResult { deleted: true })
}

/// `polish.presets.list`: the built-in presets, then the custom ones
/// `[polish] presets` names.
pub fn presets_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let loaded = daemon.config();
    let mut presets: Vec<WirePreset> = Preset::ALL
        .into_iter()
        .map(|p| WirePreset {
            id: p.as_str().to_string(),
            display_name: p.display_name().to_string(),
        })
        .collect();
    presets.extend(loaded.config.polish.presets.keys().map(|name| WirePreset {
        id: name.clone(),
        display_name: name.clone(),
    }));
    json(PresetsListResult { presets })
}

/// `polish.apps.list`.
pub fn apps_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let loaded = daemon.config();
    let mappings = loaded
        .config
        .polish
        .apps
        .iter()
        .map(|(bundle_id, entry)| AppMapping {
            bundle_id: bundle_id.clone(),
            preset: entry.preset.as_str().to_string(),
        })
        .collect();
    json(AppsListResult { mappings })
}

/// `polish.apps.set`: adds or replaces one app profile.
pub fn apps_set(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: AppMapping = params(params_value)?;
    let bundle_id = p.bundle_id.trim().to_string();
    if bundle_id.is_empty() {
        return Err(invalid("bundle_id names the app id or window class to map"));
    }
    let preset = preset_of(&p.preset)?;
    let id = bundle_id.clone();
    edit_values(daemon, |text, current| {
        let mut apps = current.polish.apps;
        let entry = apps.entry(id).or_insert(AppEntry {
            preset,
            style: None,
            post_processors: None,
            custom_preset: None,
        });
        entry.preset = preset;
        let value = serde_json::to_value(&apps).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?;
        edit::set_structured(&text, "polish.apps", &value).map_err(|e| {
            JsonRpcError::new(AppCode::InvalidParams, e.message, ErrorDetails::empty())
        })
    })?;
    json(AppMapping {
        bundle_id,
        preset: preset.as_str().to_string(),
    })
}

/// `polish.test`: the sample through the layers the mode asks for, under
/// the policy the preset, style and app id resolve to.
pub fn test(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: TestParams = params(params_value)?;
    let loaded = daemon.config();
    let preset = preset_of(&p.preset)?;
    let style = match p.style.as_deref() {
        Some(s) => Some(style_of(s)?),
        None => None,
    };
    let mode = match p.mode.as_deref() {
        Some(m) => Mode::parse(m).ok_or_else(|| {
            invalid(format!(
                "mode {m:?} is unknown; the modes are {}",
                Mode::names()
            ))
        })?,
        None => Mode::DeterministicPolish,
    };
    // The named rules replace the global set for this run, so a caller
    // sees exactly the rules it asked for and nothing else.
    let wanted: Vec<&PolishRule> = loaded
        .config
        .polish
        .rules
        .iter()
        .filter(|r| r.enabled && p.rules.iter().any(|name| name == &r.name))
        .collect();
    let applied_rules: Vec<String> = wanted.iter().map(|r| r.name.clone()).collect();
    let mut polish = loaded.config.polish.clone();
    polish.rules = wanted.into_iter().cloned().collect();
    polish.default_preset = preset;
    if let Some(style) = style {
        polish.default_style = style;
    }
    let rules = RulesConfig::from_config(&polish);
    let local: std::sync::Arc<dyn dettivo_language::provider::LocalEngine> =
        daemon.engines().local_llm();
    let outcome = pipeline::run(
        mode,
        &p.input,
        p.bundle_id.as_deref(),
        &Settings {
            replacements: &[],
            spoken_punctuation: loaded.config.dictation.spoken_punctuation,
            protect_tokens: loaded.config.dictation.protect_tokens,
            vocabulary: &loaded.config.dictation.vocabulary,
            rules: &rules,
            llm: &loaded.config.llm,
            local: Some(&local),
        },
    );
    json(TestResult {
        output: outcome.text,
        applied_rules,
        model: outcome.model,
        raw: Some(outcome.raw),
        polished: Some(outcome.polished),
        enhanced: outcome.enhanced,
        notice: outcome.notice.map(|n| PolishNotice {
            kind: n.kind.as_str().to_string(),
            reason: n.reason,
        }),
        policy_hash: Some(outcome.policy_hash),
    })
}
