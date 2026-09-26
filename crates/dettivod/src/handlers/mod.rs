//! The methods this daemon implements today: `system.*`, `config.*`,
//! `audio.devices`, `speech.*` (engines, providers, selection and models),
//! `dictation.*`, `meetings.*` (ADR 0027), `events.*`, `insert.*`,
//! `transcripts.*` over both kinds, `transfer.*`, `polish.*`, the Linux
//! `llm.*` additions, `hotkeys.status`, `hotkeys.snippet`, `hotkeys.setup`
//! and `insert.allow_self_target`.
//! Every other catalog method is answered `NOT_IMPLEMENTED` by the router
//! until its spec adds a module here.

pub mod audio;
pub mod config;
pub mod dictation;
pub mod events;
pub mod export;
pub mod hotkeys;
pub mod insert;
pub mod llm;
pub mod llm_models;
pub mod meetings;
pub mod meetings_notes;
pub mod meetings_read;
pub mod meetings_recovery;
pub mod meetings_rename;
pub mod meetings_segments;
pub mod meetings_speakers;
pub mod polish;
pub mod speech;
pub mod system;
pub mod transcripts;
pub mod transfer;

use dettivo_proto::error::JsonRpcError;
use serde_json::Value;

use crate::daemon::Daemon;
use crate::events::Sink;

/// The connection a request arrived on: its id and the channel the server
/// writes `events.notify` lines from.
#[derive(Clone)]
pub struct Ctx {
    /// The connection id.
    pub conn: u64,
    /// The connection's notification channel.
    pub notify: Sink,
    /// The peer's process id from `SO_PEERCRED`, when the kernel reported
    /// one; `insert.allow_self_target` checks it is the app's.
    pub peer_pid: Option<u32>,
}

impl Ctx {
    /// The sink a subscription sends on. The per-connection channel holds
    /// 256 lines; `buffer` is echoed to the subscriber.
    pub fn sink(&self, _buffer: u32) -> Sink {
        self.notify.clone()
    }
}

/// Shared contract decoder; retains the field path in invalid-parameter errors.
pub(super) use dettivo_proto::catalog::decode_params as params;

pub(super) fn json<T: serde::Serialize>(value: T) -> Result<Value, JsonRpcError> {
    serde_json::to_value(value).map_err(|e| {
        JsonRpcError::new(
            dettivo_proto::error::AppCode::InternalError,
            format!("cannot encode result: {e}"),
            dettivo_proto::error::ErrorDetails::empty(),
        )
    })
}

/// A resolved method. Availability and execution share this single dispatcher.
pub type Handler = fn(&Daemon, &Ctx, Value) -> Result<Value, JsonRpcError>;

/// Finds the handler without executing it.
pub fn resolve(method: &str) -> Option<Handler> {
    Some(match method {
        "dictation.start" => |daemon, _ctx, value| dictation::start(daemon, value),
        "dictation.toggle" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            dictation::toggle(daemon)
        },
        "dictation.stop" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            dictation::stop(daemon)
        },
        "dictation.cancel" => |daemon, _ctx, value| dictation::cancel(daemon, value),
        "dictation.status" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            dictation::status(daemon)
        },
        "dictation.reinsert_last" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            dictation::reinsert_last(daemon)
        },
        "meetings.start" => |daemon, _ctx, value| meetings::start(daemon, value),
        "meetings.stop" => |daemon, _ctx, value| meetings::stop(daemon, value),
        "meetings.cancel" => |daemon, _ctx, value| meetings::cancel(daemon, value),
        "meetings.status" => |daemon, _ctx, value| meetings::status(daemon, value),
        "meetings.list" => |daemon, _ctx, value| meetings_read::list(daemon, value),
        "meetings.get" => |daemon, _ctx, value| meetings_read::get(daemon, value),
        "meetings.search" => |daemon, _ctx, value| meetings_read::search(daemon, value),
        "meetings.delete" => |daemon, _ctx, value| meetings_recovery::delete(daemon, value),
        "meetings.recover" => |daemon, _ctx, value| meetings_recovery::recover(daemon, value),
        "meetings.discard" => |daemon, _ctx, value| meetings_recovery::discard(daemon, value),
        "meetings.disclosure.get" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            meetings_recovery::disclosure_get(daemon)
        },
        "meetings.disclosure.acknowledge" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            meetings_recovery::disclosure_acknowledge(daemon)
        },
        "meetings.diarize" => |daemon, _ctx, value| meetings_speakers::diarize(daemon, value),
        "meetings.speakers.list" => |daemon, _ctx, value| meetings_speakers::list(daemon, value),
        "meetings.speakers.rename" => {
            |daemon, _ctx, value| meetings_speakers::rename(daemon, value)
        }
        "meetings.speakers.suggest" => {
            |daemon, _ctx, value| meetings_speakers::suggest(daemon, value)
        }
        "meetings.notes.get" => |daemon, _ctx, value| meetings_notes::notes_get(daemon, value),
        "meetings.notes.set" => |daemon, _ctx, value| meetings_notes::notes_set(daemon, value),
        "meetings.analyze" => |daemon, _ctx, value| meetings_notes::analyze(daemon, value),
        "meetings.analysis.get" => {
            |daemon, _ctx, value| meetings_notes::analysis_get(daemon, value)
        }
        "meetings.rename" => |daemon, _ctx, value| meetings_rename::rename(daemon, value),
        "meetings.segments" => |daemon, _ctx, value| meetings_segments::segments(daemon, value),
        "events.subscribe" => |daemon, ctx, value| events::subscribe(daemon, ctx, value),
        "events.unsubscribe" => |daemon, ctx, value| events::unsubscribe(daemon, ctx, value),
        "system.ping" => |_daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            system::ping()
        },
        "system.diagnostics" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            system::diagnostics(daemon)
        },
        "system.health" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            system::health(daemon)
        },
        "system.version" => |_daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            system::version()
        },
        "system.capabilities" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            system::capabilities(daemon)
        },
        "config.get" => |daemon, _ctx, value| config::get(daemon, value),
        "config.set" => |daemon, _ctx, value| config::set(daemon, value),
        "config.unset" => |daemon, _ctx, value| config::unset(daemon, value),
        "config.validate" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            config::validate(daemon)
        },
        "config.path" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            config::path(daemon)
        },
        "config.print_default" => |_daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            config::print_default()
        },
        "config.keys" => |_daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            config::keys()
        },
        "speech.engines" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            speech::engines(daemon)
        },
        "audio.devices" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            audio::devices(daemon)
        },
        "speech.providers.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            speech::providers_list(daemon)
        },
        "speech.selection.get" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            speech::selection_get(daemon)
        },
        "speech.selection.set" => |daemon, _ctx, value| speech::selection_set(daemon, value),
        "speech.models.status" => |daemon, _ctx, value| speech::models_status(daemon, value),
        "speech.models.download" => |daemon, _ctx, value| speech::models_download(daemon, value),
        "speech.models.cancel" => |daemon, _ctx, value| speech::models_cancel(daemon, value),
        "speech.models.delete" => |daemon, _ctx, value| speech::models_delete(daemon, value),
        "insert.perform" => |daemon, _ctx, value| insert::perform(daemon, value),
        "insert.undo" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            insert::undo(daemon)
        },
        "insert.target" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            insert::target(daemon)
        },
        "transcripts.list" => |daemon, _ctx, value| transcripts::list(daemon, value),
        "transcripts.get" => |daemon, _ctx, value| transcripts::get(daemon, value),
        "transcripts.latest" => |daemon, _ctx, value| transcripts::latest(daemon, value),
        "transcripts.search" => |daemon, _ctx, value| transcripts::search(daemon, value),
        "transcripts.import" => |daemon, _ctx, value| export::import(daemon, value),
        "transcripts.export" => |daemon, _ctx, value| export::export(daemon, value),
        "transcripts.delete" => |daemon, _ctx, value| transcripts::delete(daemon, value),
        "transcripts.rerun" => |daemon, _ctx, value| transcripts::rerun(daemon, value),
        "transcripts.stats" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            transcripts::stats(daemon)
        },
        "transcripts.cancel" => |daemon, _ctx, value| transcripts::cancel(daemon, value),
        "transfer.begin" => |daemon, _ctx, value| transfer::begin(daemon, value),
        "transfer.chunk" => |daemon, _ctx, value| transfer::chunk(daemon, value),
        "transfer.pull" => |daemon, _ctx, value| transfer::pull(daemon, value),
        "transfer.commit" => |daemon, _ctx, value| transfer::commit(daemon, value),
        "transfer.cancel" => |daemon, _ctx, value| transfer::cancel(daemon, value),
        "hotkeys.status" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            hotkeys::status(daemon)
        },
        "hotkeys.snippet" => |daemon, _ctx, value| hotkeys::snippet(daemon, value),
        "hotkeys.setup" => |daemon, _ctx, value| hotkeys::setup(daemon, value),
        "insert.allow_self_target" => {
            |daemon, ctx, value| insert::allow_self_target(daemon, ctx, value)
        }
        "polish.rules.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            polish::rules_list(daemon)
        },
        "polish.rules.create" => |daemon, _ctx, value| polish::rules_create(daemon, value),
        "polish.rules.update" => |daemon, _ctx, value| polish::rules_update(daemon, value),
        "polish.rules.delete" => |daemon, _ctx, value| polish::rules_delete(daemon, value),
        "polish.presets.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            polish::presets_list(daemon)
        },
        "polish.test" => |daemon, _ctx, value| polish::test(daemon, value),
        "polish.apps.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            polish::apps_list(daemon)
        },
        "polish.apps.set" => |daemon, _ctx, value| polish::apps_set(daemon, value),
        "llm.providers.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            llm::providers_list(daemon)
        },
        "llm.endpoints.trust" => |daemon, _ctx, value| llm::endpoints_trust(daemon, value),
        "llm.endpoints.list" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            llm::endpoints_list(daemon)
        },
        "llm.models.status" => |daemon, _ctx, value| llm_models::models_status(daemon, value),
        "llm.models.download" => |daemon, _ctx, value| llm_models::models_download(daemon, value),
        "llm.models.cancel" => |daemon, _ctx, value| llm_models::models_cancel(daemon, value),
        "llm.models.delete" => |daemon, _ctx, value| llm_models::models_delete(daemon, value),
        "llm.engine.status" => |daemon, _ctx, value| {
            let _: dettivo_proto::catalog::NoParams = params(value)?;
            llm_models::engine_status(daemon)
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatch_coverage_matches_the_catalog() {
        for spec in dettivo_proto::catalog::METHODS {
            let expected = spec.status == dettivo_proto::catalog::MethodStatus::Implemented
                && !spec.name.starts_with("automation.");
            assert_eq!(resolve(spec.name).is_some(), expected, "{}", spec.name);
        }
        assert!(resolve("missing.method").is_none());
    }
}
