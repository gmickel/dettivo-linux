//! The method catalog: every method name the contract defines, whether
//! Linux implements or reserves it, and a way to run any method's params or
//! result through its typed shape by name. The fixture suite uses it to
//! prove every fixture round-trips; a router uses it to answer reserved
//! methods with `NOT_IMPLEMENTED` before it knows anything else.

use crate::methods::{
    audio, automation, config, dictation, hotkeys, insert, llm, meetings, meetings_notes,
    meetings_segments, polish, speakers, speech, system, transcripts, transfer,
};
use crate::{events, reserved};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Whether Linux implements a method or reserves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodStatus {
    /// Implemented on Linux; the fixture suite carries a success case.
    Implemented,
    /// Reserved: the router answers `NOT_IMPLEMENTED`.
    Reserved,
}

/// One entry in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodSpec {
    /// Dotted method name, for example `"system.health"`.
    pub name: &'static str,
    /// Linux status.
    pub status: MethodStatus,
    /// Whether the method exists only on Linux (a registered addition).
    pub linux_addition: bool,
}

impl MethodSpec {
    /// The namespace, everything before the first dot.
    pub fn namespace(&self) -> &'static str {
        self.name.split('.').next().unwrap_or(self.name)
    }
}

/// Params for the methods that take none. An empty object on the wire;
/// anything else is rejected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// Why a catalog lookup or round trip failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The method name is not in the catalog.
    UnknownMethod(String),
    /// The method is reserved and has no result shape.
    ReservedHasNoResult(String),
    /// The value did not match the method's shape.
    Shape {
        /// The method being checked.
        method: String,
        /// Which side failed.
        side: &'static str,
        /// serde's message, naming the field.
        message: String,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMethod(m) => write!(f, "unknown method {m:?}"),
            Self::ReservedHasNoResult(m) => write!(f, "{m} is reserved and has no result shape"),
            Self::Shape {
                method,
                side,
                message,
            } => write!(f, "{method} {side} does not match its shape: {message}"),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Decodes a handler's parameters once while retaining field-specific diagnostics.
pub fn decode_params<T: DeserializeOwned>(value: Value) -> Result<T, crate::error::JsonRpcError> {
    serde_path_to_error::deserialize(value).map_err(|e| {
        crate::error::JsonRpcError::new(
            crate::error::AppCode::InvalidParams,
            e.to_string(),
            crate::error::ErrorDetails::empty(),
        )
    })
}

fn round_trip<T: DeserializeOwned + Serialize>(
    method: &str,
    side: &'static str,
    value: &Value,
) -> Result<Value, CatalogError> {
    // serde_path_to_error prefixes the message with the path to the field
    // that failed (`meeting_id: not a lowercase UUID string`), so a drifted
    // fixture or message names what drifted.
    let typed: T =
        serde_path_to_error::deserialize(value.clone()).map_err(|e| CatalogError::Shape {
            method: method.to_string(),
            side,
            message: e.to_string(),
        })?;
    serde_json::to_value(&typed).map_err(|e| CatalogError::Shape {
        method: method.to_string(),
        side,
        message: e.to_string(),
    })
}

macro_rules! catalog {
    (
        implemented { $( $iname:literal => ($ip:ty, $ir:ty) $(, linux: $lx:literal)? ; )* }
        reserved { $( $rname:literal => $rp:ty ; )* }
    ) => {
        /// Every method the contract and the Linux register define.
        pub const METHODS: &[MethodSpec] = &[
            $( MethodSpec { name: $iname, status: MethodStatus::Implemented, linux_addition: false $( || $lx )? }, )*
            $( MethodSpec { name: $rname, status: MethodStatus::Reserved, linux_addition: false }, )*
        ];

        /// Runs a method's params through its typed shape and back.
        pub fn round_trip_params(method: &str, value: &Value) -> Result<Value, CatalogError> {
            match method {
                $( $iname => round_trip::<$ip>(method, "params", value), )*
                $( $rname => round_trip::<$rp>(method, "params", value), )*
                other => Err(CatalogError::UnknownMethod(other.to_string())),
            }
        }

        /// Runs a method's result through its typed shape and back.
        pub fn round_trip_result(method: &str, value: &Value) -> Result<Value, CatalogError> {
            match method {
                $( $iname => round_trip::<$ir>(method, "result", value), )*
                $( $rname => Err(CatalogError::ReservedHasNoResult(method.to_string())), )*
                other => Err(CatalogError::UnknownMethod(other.to_string())),
            }
        }
    };
}

catalog! {
    implemented {
        "system.ping" => (NoParams, system::PingResult);
        "system.diagnostics" => (NoParams, system::DiagnosticsResult), linux: true;
        "system.health" => (NoParams, system::HealthResult);
        "system.version" => (NoParams, system::VersionResult);
        "system.capabilities" => (NoParams, system::CapabilitiesResult);
        "dictation.start" => (dictation::StartParams, dictation::StartResult);
        "dictation.toggle" => (NoParams, dictation::ToggleResult), linux: true;
        "dictation.stop" => (NoParams, dictation::StopResult);
        "dictation.cancel" => (dictation::CancelParams, dictation::CancelResult);
        "dictation.status" => (NoParams, dictation::StatusResult);
        "dictation.reinsert_last" => (NoParams, dictation::ReinsertLastResult), linux: true;
        "meetings.start" => (meetings::StartParams, meetings::StartResult);
        "meetings.stop" => (meetings::MeetingIdParams, meetings::StopResult);
        "meetings.cancel" => (meetings::MeetingIdParams, meetings::CancelResult);
        "meetings.status" => (meetings::MeetingIdParams, meetings::StatusResult);
        "meetings.list" => (meetings::ListParams, meetings::ListResult);
        "meetings.get" => (meetings::GetParams, meetings::GetResult);
        "meetings.search" => (meetings::SearchParams, meetings::SearchResult);
        "meetings.delete" => (meetings::DeleteParams, meetings::DeleteResult);
        "meetings.recover" => (meetings::RecoverParams, meetings::RecoverResult), linux: true;
        "meetings.discard" => (meetings::RecoverParams, meetings::DiscardResult), linux: true;
        "meetings.disclosure.get" => (NoParams, meetings::DisclosureResult), linux: true;
        "meetings.disclosure.acknowledge" => (NoParams, meetings::DisclosureResult), linux: true;
        "meetings.diarize" => (speakers::DiarizeParams, speakers::DiarizeResult), linux: true;
        "meetings.speakers.list" => (speakers::SpeakersListParams, speakers::SpeakersListResult), linux: true;
        "meetings.speakers.rename" => (speakers::RenameParams, speakers::RenameResult), linux: true;
        "meetings.speakers.suggest" => (speakers::SuggestParams, speakers::SuggestResult), linux: true;
        "meetings.notes.get" => (meetings_notes::NotesGetParams, meetings_notes::NotesResult), linux: true;
        "meetings.notes.set" => (meetings_notes::NotesSetParams, meetings_notes::NotesResult), linux: true;
        "meetings.analyze" => (meetings_notes::AnalyzeParams, meetings_notes::AnalyzeResult), linux: true;
        "meetings.analysis.get" => (meetings_notes::AnalysisGetParams, meetings_notes::AnalysisGetResult), linux: true;
        "meetings.rename" => (meetings_notes::MeetingRenameParams, meetings_notes::MeetingRenameResult), linux: true;
        "meetings.segments" => (meetings_segments::SegmentsParams, meetings_segments::SegmentsResult), linux: true;
        "transcripts.list" => (transcripts::ListParams, transcripts::ListResult);
        "transcripts.get" => (transcripts::GetParams, transcripts::GetResult);
        "transcripts.latest" => (transcripts::LatestParams, transcripts::LatestResult);
        "transcripts.search" => (transcripts::SearchParams, transcripts::SearchResult);
        "transcripts.import" => (transcripts::ImportParams, transcripts::ImportResult);
        "transcripts.export" => (transcripts::ExportParams, transcripts::ExportResult);
        "transcripts.delete" => (transcripts::DeleteParams, transcripts::DeleteResult), linux: true;
        "transcripts.rerun" => (transcripts::RerunParams, transcripts::RerunResult), linux: true;
        "transcripts.stats" => (NoParams, transcripts::StatsResult), linux: true;
        "transcripts.cancel" => (transcripts::CancelParams, transcripts::CancelResult), linux: true;
        "insert.perform" => (insert::PerformParams, insert::PerformResult);
        "polish.rules.list" => (NoParams, polish::RulesListResult);
        "polish.rules.create" => (polish::RulesCreateParams, polish::RulesCreateResult);
        "polish.rules.update" => (polish::RulesUpdateParams, polish::RulesUpdateResult);
        "polish.rules.delete" => (polish::RulesDeleteParams, polish::RulesDeleteResult);
        "polish.presets.list" => (NoParams, polish::PresetsListResult);
        "polish.test" => (polish::TestParams, polish::TestResult);
        "polish.apps.list" => (NoParams, polish::AppsListResult);
        "polish.apps.set" => (polish::AppMapping, polish::AppMapping);
        "llm.providers.list" => (NoParams, llm::ProvidersListResult), linux: true;
        "llm.endpoints.trust" => (llm::EndpointsTrustParams, llm::EndpointsTrustResult), linux: true;
        "llm.endpoints.list" => (NoParams, llm::EndpointsListResult), linux: true;
        "llm.models.status" => (llm::ModelsStatusParams, llm::ModelsStatusResult), linux: true;
        "llm.models.download" => (llm::ModelParams, speech::ModelStatus), linux: true;
        "llm.models.cancel" => (llm::ModelParams, speech::ModelStatus), linux: true;
        "llm.models.delete" => (llm::ModelDeleteParams, llm::ModelDeleteResult), linux: true;
        "llm.engine.status" => (NoParams, llm::EngineStatusResult), linux: true;
        "automation.macros.list" => (NoParams, automation::MacrosListResult);
        "automation.macros.upsert" => (automation::MacrosUpsertParams, automation::MacrosUpsertResult);
        "automation.macros.delete" => (automation::MacrosDeleteParams, automation::MacrosDeleteResult);
        "automation.macros.preview" => (automation::MacrosPreviewParams, automation::MacrosPreviewResult);
        "automation.macros.run" => (automation::MacrosRunParams, automation::MacrosRunResult);
        "automation.macros.audit.list" => (automation::AuditListParams, automation::AuditListResult);
        "events.subscribe" => (events::SubscribeParams, events::SubscribeResult);
        "events.unsubscribe" => (events::UnsubscribeParams, events::UnsubscribeResult);
        "transfer.begin" => (transfer::BeginParams, transfer::BeginResult);
        "transfer.chunk" => (transfer::ChunkParams, transfer::ChunkResult);
        "transfer.pull" => (transfer::PullParams, transfer::PullResult);
        "transfer.commit" => (transfer::CommitParams, transfer::CommitResult);
        "transfer.cancel" => (transfer::CancelParams, transfer::CancelResult);
        "speech.providers.list" => (NoParams, speech::ProvidersListResult), linux: true;
        "speech.selection.get" => (NoParams, speech::SelectionResult), linux: true;
        "speech.selection.set" => (speech::SelectionSetParams, speech::SelectionResult), linux: true;
        "speech.engines" => (NoParams, speech::EnginesResult), linux: true;
        "speech.models.status" => (speech::ModelsStatusParams, speech::ModelsStatusResult), linux: true;
        "speech.models.download" => (speech::ModelParams, speech::ModelStatus), linux: true;
        "speech.models.cancel" => (speech::ModelParams, speech::ModelStatus), linux: true;
        "speech.models.delete" => (speech::ModelDeleteParams, speech::ModelDeleteResult), linux: true;
        "config.get" => (config::GetParams, config::GetResult), linux: true;
        "config.set" => (config::SetParams, config::SetResult), linux: true;
        "config.unset" => (config::UnsetParams, config::UnsetResult), linux: true;
        "config.validate" => (NoParams, config::ValidateResult), linux: true;
        "config.path" => (NoParams, config::PathResult), linux: true;
        "config.print_default" => (NoParams, config::PrintDefaultResult), linux: true;
        "config.keys" => (NoParams, config::KeysResult), linux: true;
        "audio.devices" => (NoParams, audio::DevicesResult), linux: true;
        "insert.undo" => (NoParams, insert::UndoResult), linux: true;
        "insert.target" => (NoParams, insert::TargetResult), linux: true;
        "hotkeys.status" => (NoParams, hotkeys::StatusResult), linux: true;
        "hotkeys.snippet" => (hotkeys::SnippetParams, hotkeys::SnippetResult), linux: true;
        "hotkeys.setup" => (hotkeys::SetupParams, hotkeys::SetupResult), linux: true;
        "insert.allow_self_target" => (insert::AllowSelfTargetParams, insert::AllowSelfTargetResult), linux: true;
    }
    reserved {
        "knowledge.search" => reserved::UndocumentedReservedParams;
        "knowledge.ask" => reserved::UndocumentedReservedParams;
        "meeting.templates.list" => reserved::UndocumentedReservedParams;
        "meeting.templates.set_default" => reserved::UndocumentedReservedParams;
        "retention.policy.get" => reserved::UndocumentedReservedParams;
        "retention.policy.set" => reserved::UndocumentedReservedParams;
        "automation.jobs.create" => reserved::AutomationJobsCreateParams;
        "automation.jobs.list" => reserved::AutomationJobsListParams;
        "automation.providers.list" => NoParams;
        "automation.providers.connect" => reserved::AutomationProvidersConnectParams;
        "automation.providers.disconnect" => reserved::AutomationProvidersDisconnectParams;
    }
}

/// Looks a method up by name.
pub fn lookup(method: &str) -> Option<&'static MethodSpec> {
    METHODS.iter().find(|m| m.name == method)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_names_are_unique_and_dotted() {
        let mut seen = std::collections::HashSet::new();
        for m in METHODS {
            assert!(m.name.contains('.'), "{}", m.name);
            assert!(seen.insert(m.name), "duplicate {}", m.name);
        }
        assert_eq!(
            lookup("system.ping").map(|m| m.status),
            Some(MethodStatus::Implemented)
        );
        assert_eq!(
            lookup("knowledge.ask").map(|m| m.status),
            Some(MethodStatus::Reserved)
        );
        assert!(lookup("speech.selection.get").unwrap().linux_addition);
        assert!(lookup("meetings.segments").unwrap().linux_addition);
        assert!(lookup("nope.method").is_none());
    }

    #[test]
    fn reserved_methods_match_the_reserved_module() {
        let from_catalog: Vec<&str> = METHODS
            .iter()
            .filter(|m| m.status == MethodStatus::Reserved)
            .map(|m| m.name)
            .collect();
        let from_module: Vec<&str> = reserved::ReservedMethod::ALL
            .iter()
            .map(|m| m.as_wire_str())
            .collect();
        assert_eq!(from_catalog, from_module);
    }

    #[test]
    fn round_trip_rejects_unknown_fields_by_name() {
        let err = round_trip_params("system.ping", &json!({"stray": 1})).unwrap_err();
        assert!(err.to_string().contains("stray"), "{err}");
        let err = round_trip_result("knowledge.ask", &json!({})).unwrap_err();
        assert!(matches!(err, CatalogError::ReservedHasNoResult(_)));
    }
}
