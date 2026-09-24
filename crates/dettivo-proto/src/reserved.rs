//! Reserved namespaces and methods: enumerated with their reserved shapes
//! so a router can answer `NOT_IMPLEMENTED` with the right body
//! (`docs/api/dettivo-ipc-v1.md` section 9; extended per `docs/api/
//! linux-deltas.md`).
//!
//! Reserved namespaces (every method under them is reserved):
//! `knowledge.*`, `meeting.templates.*`, `retention.*`.
//!
//! Reserved individual methods: `automation.jobs.*` and
//! `automation.providers.*` — implemented on macOS, but reserved on Linux
//! v1 per the Linux delta register (no email/OAuth provider account model
//! shipped here yet). Their request shapes are still known from section
//! 8.7, so they are typed as the reserved request shape; every reserved
//! method's *response* is `NOT_IMPLEMENTED` regardless. `meetings.delete`,
//! reserved on macOS, is implemented here (`methods::meetings`).

use crate::id::Id;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use serde_json::Value;

/// One reserved method name, spanning both the macOS-reserved namespaces
/// and the Linux-only reservations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservedMethod {
    /// `knowledge.search`.
    KnowledgeSearch,
    /// `knowledge.ask`.
    KnowledgeAsk,
    /// `meeting.templates.list`.
    MeetingTemplatesList,
    /// `meeting.templates.set_default`.
    MeetingTemplatesSetDefault,
    /// `retention.policy.get`.
    RetentionPolicyGet,
    /// `retention.policy.set`.
    RetentionPolicySet,
    /// `automation.jobs.create`. Reserved on Linux only.
    AutomationJobsCreate,
    /// `automation.jobs.list`. Reserved on Linux only.
    AutomationJobsList,
    /// `automation.providers.list`. Reserved on Linux only.
    AutomationProvidersList,
    /// `automation.providers.connect`. Reserved on Linux only.
    AutomationProvidersConnect,
    /// `automation.providers.disconnect`. Reserved on Linux only.
    AutomationProvidersDisconnect,
}

impl ReservedMethod {
    /// The dotted wire method name.
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::KnowledgeSearch => "knowledge.search",
            Self::KnowledgeAsk => "knowledge.ask",
            Self::MeetingTemplatesList => "meeting.templates.list",
            Self::MeetingTemplatesSetDefault => "meeting.templates.set_default",
            Self::RetentionPolicyGet => "retention.policy.get",
            Self::RetentionPolicySet => "retention.policy.set",
            Self::AutomationJobsCreate => "automation.jobs.create",
            Self::AutomationJobsList => "automation.jobs.list",
            Self::AutomationProvidersList => "automation.providers.list",
            Self::AutomationProvidersConnect => "automation.providers.connect",
            Self::AutomationProvidersDisconnect => "automation.providers.disconnect",
        }
    }

    /// Every reserved method.
    pub const ALL: &'static [ReservedMethod] = &[
        Self::KnowledgeSearch,
        Self::KnowledgeAsk,
        Self::MeetingTemplatesList,
        Self::MeetingTemplatesSetDefault,
        Self::RetentionPolicyGet,
        Self::RetentionPolicySet,
        Self::AutomationJobsCreate,
        Self::AutomationJobsList,
        Self::AutomationProvidersList,
        Self::AutomationProvidersConnect,
        Self::AutomationProvidersDisconnect,
    ];
}

/// `automation.jobs.create` reserved params, per the macOS-documented
/// request shape (section 8.7) — still a valid request on Linux; the
/// response is `NOT_IMPLEMENTED`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationJobsCreateParams {
    /// What triggered this job.
    pub trigger: String,
    /// Meeting id, when `trigger="analysis_ready"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting_id: Option<Id>,
    /// Whether to force re-running an already-completed job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// `automation.jobs.list` reserved params.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationJobsListParams {
    /// Maximum entries to return.
    pub limit: u32,
}

/// `automation.providers.connect` reserved params, per the
/// macOS-documented request shape (section 8.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationProvidersConnectParams {
    /// Which provider kind to connect.
    pub provider: String,
    /// Sender email address.
    pub sender_email: String,
    /// Optional display name for the sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_name: Option<String>,
    /// OAuth access token, required for `gmail`/`microsoft_graph`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// OAuth refresh token, optional for `gmail`/`microsoft_graph`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// SMTP host, for `smtp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_host: Option<String>,
    /// SMTP port, for `smtp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_port: Option<u16>,
    /// SMTP username, for `smtp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_username: Option<String>,
    /// SMTP password, for `smtp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_password: Option<String>,
    /// Whether to use TLS, for `smtp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_use_tls: Option<bool>,
}

/// `automation.providers.disconnect` reserved params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationProvidersDisconnectParams {
    /// The provider connection to remove.
    pub provider_id: Id,
}

/// Reserved params with no request shape documented on macOS
/// (`knowledge.*`, `meeting.templates.*`, `retention.*`): the wire method
/// still accepts an arbitrary JSON object, passed through untyped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UndocumentedReservedParams(pub Map<String, Value>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reserved_method_has_a_wire_name() {
        for method in ReservedMethod::ALL {
            assert!(method.as_wire_str().contains('.'));
        }
    }
}
