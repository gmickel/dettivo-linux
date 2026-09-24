//! `transcripts.export` renders one item, a date range or everything into
//! a download transfer the client began (a meeting in its five formats);
//! `transcripts.import` consumes a committed upload as an audio import job
//! (into a dictation item or a meeting) or an archive restore.

use dettivo_proto::capabilities::ExportFormat;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::transcripts::{
    ExportParams, ExportResult, ExportScope, ImportParams, ImportResult,
};
use dettivo_proto::runtime::RefKind;
use dettivo_storage::dictations::ListFilter;
use dettivo_storage::export::{self, Format};
use dettivo_storage::meeting_export::{self, MeetingFormat, Options};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::handlers::transcripts::lookup;
use crate::history::store_error;

fn invalid(message: String) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

/// The wire spelling of a format.
fn format_name(format: ExportFormat) -> String {
    serde_json::to_value(format)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// `transcripts.export`.
pub fn export(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ExportParams = params(params_value)?;
    let kind = p
        .reference
        .as_ref()
        .map(|r| r.kind)
        .unwrap_or(RefKind::Dictation);
    if kind == RefKind::Meeting {
        return export_meeting(daemon, &p);
    }
    let format = match p.format {
        ExportFormat::Txt => Format::Txt,
        ExportFormat::Md => Format::Md,
        ExportFormat::Json => Format::Json,
        ExportFormat::Zip => Format::Zip,
        other => {
            return Err(invalid(format!(
                "{} is not a dictation export format",
                format_name(other)
            )));
        }
    };
    let scope = p.scope.unwrap_or(ExportScope::Item);
    let history = daemon.history();
    let (items, stem) = match scope {
        ExportScope::Item => {
            let reference = p
                .reference
                .as_ref()
                .ok_or_else(|| invalid("ref is required for scope item".into()))?;
            let item = lookup(daemon, reference)?;
            let stem = format!("dictation-{}", &item.id[..8]);
            (vec![item], stem)
        }
        ExportScope::Range => {
            let since = p
                .from
                .clone()
                .ok_or_else(|| invalid("from is required for scope range".into()))?;
            let until =
                p.to.clone()
                    .ok_or_else(|| invalid("to is required for scope range".into()))?;
            for (name, value) in [("from", &since), ("to", &until)] {
                if dettivo_storage::time::unix_from_iso(value).is_none() {
                    return Err(invalid(format!(
                        "{name} {value:?} is not an ISO 8601 date or time"
                    )));
                }
            }
            let filter = ListFilter {
                since: Some(since),
                until: Some(until),
                ..ListFilter::default()
            };
            let items = history
                .with_store(|store| store.all(&filter))
                .map_err(store_error)?;
            (items, "dictations".to_string())
        }
        ExportScope::All => {
            let items = history
                .with_store(|store| store.all(&ListFilter::default()))
                .map_err(store_error)?;
            (items, "dictations".to_string())
        }
    };
    let bytes = export::render(format, &items);
    daemon.transfers().bind_export(&p.transfer_id, &bytes)?;
    tracing::info!(
        items = items.len(),
        format = format.as_str(),
        "history exported"
    );
    json(ExportResult {
        transfer_id: p.transfer_id,
        content_type: format.content_type().to_string(),
        filename: format.filename(&stem),
    })
}

/// One meeting in one of the contract's meeting formats.
fn export_meeting(daemon: &Daemon, p: &ExportParams) -> Result<Value, JsonRpcError> {
    let reference = p
        .reference
        .as_ref()
        .ok_or_else(|| invalid("ref is required for a meeting export".into()))?;
    let format = match p.format {
        ExportFormat::Txt => MeetingFormat::Txt,
        ExportFormat::Md => MeetingFormat::Md,
        ExportFormat::Json => MeetingFormat::Json,
        ExportFormat::Srt => MeetingFormat::Srt,
        ExportFormat::Vtt => MeetingFormat::Vtt,
        other => {
            return Err(invalid(format!(
                "{} is not a meeting export format; the formats are {}",
                format_name(other),
                MeetingFormat::ALL
                    .iter()
                    .map(|f| f.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };
    let row = crate::handlers::meetings::lookup(daemon, reference.id.as_str())?;
    let options = Options {
        notes_override: p.notes_override.clone(),
        raw: p.raw.unwrap_or(false),
    };
    let bytes = meeting_export::render_with(format, &row, &options);
    daemon.transfers().bind_export(&p.transfer_id, &bytes)?;
    tracing::info!(format = format.as_str(), "meeting exported");
    json(ExportResult {
        transfer_id: p.transfer_id.clone(),
        content_type: format.content_type().to_string(),
        filename: format.filename(),
    })
}

/// `transcripts.import`.
pub fn import(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ImportParams = params(params_value)?;
    if p.target_kind == RefKind::Meeting {
        // The same disclosure gate as `meetings.start` (FR-G9).
        let disclosure = if p.acknowledge_meeting_disclosure == Some(true) {
            daemon
                .meetings()
                .acknowledge()
                .map_err(|e| JsonRpcError::new(AppCode::InternalError, e, ErrorDetails::empty()))?
        } else {
            daemon.meetings().disclosure()
        };
        if !disclosure.acknowledged {
            return Err(JsonRpcError::new(
                AppCode::Conflict,
                "Meeting disclosure acknowledgement required",
                ErrorDetails::conflict_kind("meetingDisclosureRequired"),
            ));
        }
    }
    if daemon.jobs().import_in_progress() {
        return Err(JsonRpcError::new(
            AppCode::Conflict,
            "Import already in progress",
            ErrorDetails::conflict_kind("importAlreadyInProgress"),
        ));
    }
    let upload = daemon.transfers().upload(&p.transfer_id)?;
    let result: ImportResult = daemon.jobs().start_import(daemon, upload, &p)?;
    json(result)
}
