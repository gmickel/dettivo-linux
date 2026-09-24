//! `automation.macros.*`: dictation macros (`docs/api/dettivo-ipc-v1.md`
//! section 8.7). `automation.jobs.*` and `automation.providers.*` are
//! reserved on Linux (`reserved.rs`, `docs/api/linux-deltas.md`) even
//! though the macOS contract implements them; `system.capabilities.
//! automation.{jobs,providers}` are `false` accordingly.

use serde::{Deserialize, Serialize};

/// How much confirmation a macro action requires before running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyPolicy {
    /// Always ask for confirmation.
    AlwaysConfirm,
    /// Ask once, then remember the choice.
    FirstUseConfirm,
    /// Never ask; run automatically.
    AutoAllow,
}

/// What kind of action a macro performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Insert literal text.
    InsertText,
    /// Run a named app command.
    AppCommand,
    /// Run an allow-listed shell command.
    ShellCommand,
}

/// A macro's action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// What kind of action this is.
    pub kind: ActionKind,
    /// Text, command id, or executable, depending on `kind`.
    pub target: String,
    /// Positional arguments for `app_command`/`shell_command`.
    pub arguments: Vec<String>,
    /// Human-readable preview of what running this action does.
    pub preview: String,
}

/// One configured dictation macro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Macro {
    /// Macro id.
    pub id: String,
    /// Trigger phrase.
    pub trigger: String,
    /// Whether the macro is currently active.
    pub enabled: bool,
    /// Confirmation policy before running.
    pub safety_policy: SafetyPolicy,
    /// What the macro does when triggered.
    pub action: Action,
}

/// App/shell command allowlists gating `app_command`/`shell_command`
/// actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allowlist {
    /// Allowed named app commands.
    pub app_commands: Vec<String>,
    /// Allowed shell executables.
    pub shell_commands: Vec<String>,
}

/// The full macro configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroConfig {
    /// Whether the macro system is enabled at all.
    pub enabled: bool,
    /// Configured macros.
    pub macros: Vec<Macro>,
    /// Command allowlists.
    pub allowlist: Allowlist,
    /// Macro ids the user has already confirmed under
    /// `first_use_confirm`.
    pub remembered_macro_ids: Vec<String>,
}

/// `automation.macros.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosListResult {
    /// The full macro configuration.
    pub config: MacroConfig,
}

/// `automation.macros.upsert` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosUpsertParams {
    /// Existing macro id to update, or `None` to create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_id: Option<String>,
    /// Trigger phrase.
    pub trigger: String,
    /// Action kind.
    pub action_kind: ActionKind,
    /// Action target.
    pub target: String,
    /// Action arguments.
    pub arguments: Vec<String>,
    /// Confirmation policy.
    pub safety_policy: SafetyPolicy,
    /// Whether this macro is enabled.
    pub enabled: bool,
    /// Whether the overall macro system stays enabled.
    pub config_enabled: bool,
    /// App commands to allow.
    pub allowlist_app_commands: Vec<String>,
    /// Shell commands to allow.
    pub allowlist_shell_commands: Vec<String>,
}

/// `automation.macros.upsert` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosUpsertResult {
    /// The updated full macro configuration.
    pub config: MacroConfig,
}

/// `automation.macros.delete` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosDeleteParams {
    /// The macro to delete.
    pub macro_id: String,
}

/// `automation.macros.delete` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosDeleteResult {
    /// Always `true` on success.
    pub deleted: bool,
    /// The updated full macro configuration.
    pub config: MacroConfig,
}

/// `automation.macros.preview` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosPreviewParams {
    /// Sample transcript text to preview against.
    pub transcript: String,
}

/// Outcome of previewing (or running) a macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroRunStatus {
    /// Preview only, not executed.
    Preview,
    /// Executed.
    Run,
    /// Blocked by policy/confirmation.
    Blocked,
    /// Execution failed.
    Failed,
    /// Cancelled mid-run.
    Cancelled,
    /// Dropped (e.g. no matching trigger).
    Dropped,
}

/// A macro preview or run outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroOutcome {
    /// The macro that matched, when any.
    pub r#macro: Option<Macro>,
    /// Outcome status.
    pub status: MacroRunStatus,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Whether the action was actually applied.
    pub applied: bool,
    /// Human-readable preview of the action.
    pub action_preview: String,
    /// Transcript text after macro substitution.
    pub transformed_text: String,
    /// Optional status message.
    pub message: Option<String>,
}

/// `automation.macros.preview` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosPreviewResult {
    /// The preview outcome.
    pub preview: MacroOutcome,
}

/// `automation.macros.run` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosRunParams {
    /// Trigger phrase to match.
    pub trigger: String,
    /// Transcript text to run against.
    pub transcript: String,
    /// Whether to simulate without applying the action.
    pub dry_run: bool,
}

/// `automation.macros.run` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacrosRunResult {
    /// The run outcome.
    pub result: MacroOutcome,
}

/// What kind of audit event a macro log entry records. Distinct from
/// [`MacroRunStatus`]: the audit log records `trigger` (matched but not
/// yet resolved) instead of `preview` (section 8.7's
/// `automation.macros.audit.list`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// The trigger phrase matched.
    Trigger,
    /// Executed.
    Run,
    /// Blocked by policy/confirmation.
    Blocked,
    /// Execution failed.
    Failed,
    /// Cancelled mid-run.
    Cancelled,
    /// Dropped (e.g. no matching trigger).
    Dropped,
}

/// One audit-log entry for a macro trigger/run event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditItem {
    /// Audit entry id.
    pub id: String,
    /// The macro id involved, when known.
    pub macro_id: Option<String>,
    /// Trigger phrase that fired.
    pub trigger: String,
    /// Action kind of the macro involved.
    pub action_kind: ActionKind,
    /// Action target of the macro involved.
    pub action_target: String,
    /// What kind of audit event this is.
    pub event_kind: AuditEventKind,
    /// Optional status message.
    pub message: Option<String>,
    /// ISO 8601 timestamp of the event.
    pub created_at: String,
}

/// `automation.macros.audit.list` params.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditListParams {
    /// Maximum entries to return.
    pub limit: u32,
}

/// `automation.macros.audit.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditListResult {
    /// Audit log entries.
    pub items: Vec<AuditItem>,
}
