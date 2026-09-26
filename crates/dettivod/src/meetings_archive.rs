//! What the daemon does with a meeting row the session hands it (ADR
//! 0036): the row goes to the history store at start and on every change,
//! a completed finalisation gets its segments through the deterministic
//! Polish pass (the global `[polish]` transforms, no app rules) so
//! `transcript` is the polished join and `raw_text` the engine's words,
//! and plans the passes that follow on the row before it is stored: the
//! speaker pass when `[meetings.diarization]` says so and the analysis
//! when `[meetings.analysis] auto` is on or the meeting asked for it,
//! each marked `queued` so a client that reloads on `completed` sees what
//! runs next (ADR 0061). A settled meeting then runs the speaker pass and
//! starts the analysis it planned; a plan that cannot start is settled
//! `failed` with the reason, never left queued. The import path
//! (`transcripts.import` into a meeting) goes through the same two hooks.

use std::sync::{Arc, Mutex, RwLock};

use dettivo_core::config::Loaded;
use dettivo_core::config::polish_schema::Polish;
use dettivo_language::policy::{Backend, Context, RulesConfig, resolve};
use dettivo_language::polish;
use dettivo_proto::methods::meetings::Segment;
use dettivo_proto::methods::meetings_notes::AnalysisStatus;
use dettivo_proto::methods::speakers::DiarizationStatus;
use dettivo_storage::meetings::MeetingRow;
use dettivo_storage::retention::ArtifactPolicy;

use crate::analysis::Analysis;
use crate::diarization::Diarization;
use crate::history::History;

type CompletedHook = Arc<dyn Fn(&MeetingRow) + Send + Sync>;

/// The archive behind the meeting session.
pub struct MeetingArchive {
    history: Arc<History>,
    analysis: Arc<Analysis>,
    config: Arc<RwLock<Loaded>>,
    on_completed: Mutex<Option<CompletedHook>>,
}

impl MeetingArchive {
    /// An archive over the store, the analysis service and the
    /// configuration cell.
    pub fn new(
        history: Arc<History>,
        analysis: Arc<Analysis>,
        config: Arc<RwLock<Loaded>>,
    ) -> Self {
        Self {
            history,
            analysis,
            config,
            on_completed: Mutex::new(None),
        }
    }

    /// Installs the ordered speaker pass, invoked before analysis and retention.
    pub fn set_on_completed(&self, hook: CompletedHook) {
        *self.on_completed.lock().unwrap_or_else(|p| p.into_inner()) = Some(hook);
    }

    fn retain_import(&self, row: &MeetingRow, keep_audio: bool, policy: ArtifactPolicy) {
        let artifacts = self.history.meeting_artifacts();
        let result = if policy == ArtifactPolicy::None {
            artifacts
                .remove(&row.id)
                .and_then(|()| self.history.with_store(|s| s.clear_meeting_audio(&row.id)))
        } else {
            if policy == ArtifactPolicy::Keep {
                let dir = artifacts.dir(&row.id);
                let value = serde_json::json!({"meeting_id": row.id, "takes": [], "journal": dettivo_meeting::JOURNAL_FILE, "checkpoint_schema": dettivo_meeting::checkpoint::SCHEMA_VERSION});
                if let Err(e) =
                    std::fs::write(dir.join(dettivo_meeting::METADATA_FILE), value.to_string())
                {
                    tracing::warn!(error = %e, "meeting import manifest not written");
                }
            }
            if keep_audio {
                Ok(())
            } else {
                artifacts.remove_audio(&row.id)
            }
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "meeting import retention failed");
        }
    }

    /// Fills `polished_text` on every segment from the global transforms
    /// and makes `final_text` the polished join; `raw_text` keeps the
    /// engine's words.
    pub fn polish_segments(polish_config: &Polish, row: &mut MeetingRow) {
        Self::polish_where(polish_config, row, |_| true);
    }

    /// Polishes the segments that carry no polished text, the parts the
    /// speaker pass split (ADR 0072), and joins `final_text` again.
    pub fn polish_missing(polish_config: &Polish, row: &mut MeetingRow) {
        Self::polish_where(polish_config, row, |s| s.polished_text.is_none());
    }

    fn polish_where(
        polish_config: &Polish,
        row: &mut MeetingRow,
        wanted: impl Fn(&Segment) -> bool,
    ) {
        let rules = RulesConfig::from_config(polish_config);
        let policy = resolve(&rules, &Backend::Disabled, &Context::default());
        for segment in row.segments.iter_mut().filter(|s| wanted(s)) {
            let polished = polish::rewrite(
                &segment.text,
                &policy.transforms,
                &policy.post_processors,
                policy.style,
            );
            segment.polished_text = Some(polished);
        }
        if !row.segments.is_empty() {
            row.final_text = row.polished_join();
        }
    }
}

impl dettivo_meeting::Archive for MeetingArchive {
    fn started(&self, row: &MeetingRow) -> Result<(), String> {
        dettivo_meeting::Archive::started(self.history.as_ref(), row)
    }

    fn updated(&self, row: &MeetingRow) -> Result<(), String> {
        dettivo_meeting::Archive::updated(self.history.as_ref(), row)
    }

    /// Polishes the segments and plans the passes on the row: the speaker
    /// pass (`Diarization::planned`, the same answer `auto_start` acts on)
    /// and the analysis (`Analysis::wants_auto`, read once here) each
    /// leave `queued` on the row the finalisation stores next.
    fn completed(&self, row: &mut MeetingRow) {
        let loaded = self.config.read().unwrap_or_else(|p| p.into_inner());
        Self::polish_segments(&loaded.config.polish, row);
        tracing::info!(id = %row.id, segments = row.segments.len(), "meeting segments polished");
        let diarize = Diarization::planned(&loaded.config.meetings.diarization, row);
        if diarize {
            let mut block = row.diarization.clone().unwrap_or_default();
            block.status = DiarizationStatus::Queued;
            block.error = None;
            row.diarization = Some(block);
        }
        let analyze =
            self.analysis.wants_auto(&row.id) && row.analysis_status != AnalysisStatus::Ready;
        if analyze {
            row.analysis_status = AnalysisStatus::Queued;
            // The whole-row store that follows owns the capture, the
            // transcription and the speaker pass; the analysis columns
            // are written by their own setter, so the plan goes to the
            // store here (the row exists since the meeting started).
            if let Err(e) = self.history.with_store(|s| {
                s.set_meeting_analysis_status(&row.id, AnalysisStatus::Queued, None)
            }) {
                tracing::warn!(error = %e, "analysis: queued status not stored");
            }
        }
        tracing::info!(id = %row.id, diarize, analyze, "meeting passes planned");
    }

    /// Runs the speaker pass hook, the import retention, then the analysis
    /// the row was planned for (`queued`); a refused start settles the
    /// plan `failed`.
    fn settled(&self, row: &MeetingRow, keep_audio: bool, artifacts: ArtifactPolicy) {
        let hook = self
            .on_completed
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook(row);
        }
        let Ok(Some(current)) = self.history.with_store(|s| s.get_meeting(&row.id)) else {
            return;
        };
        if current.source_kind == "audioImport" {
            self.retain_import(&current, keep_audio, artifacts);
        }
        if current.analysis_status == AnalysisStatus::Queued
            && let Err(e) =
                self.analysis
                    .start_with_artifacts(current, true, artifacts == ArtifactPolicy::Keep)
        {
            tracing::info!(id = %row.id, refusal = ?e, "meeting analysis not started");
            self.analysis.settle_queued(&row.id, &e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::methods::meetings::{Segment, SegmentSource};

    #[test]
    fn the_polish_pass_fills_every_segment_and_the_join() {
        let mut row = MeetingRow::new();
        for (i, text) in ["um so the budget is agreed", "we ship on thursday"]
            .iter()
            .enumerate()
        {
            row.segments.push(Segment {
                index: i as u32,
                start_ms: i as u64 * 1000,
                end_ms: i as u64 * 1000 + 900,
                text: (*text).into(),
                speaker: None,
                speaker_id: None,
                speaker_confidence: None,
                source_type: SegmentSource::Microphone,
                words: Vec::new(),
                gap_before_ms: None,
                polished_text: None,
            });
        }
        row.raw_text = "um so the budget is agreed we ship on thursday".into();
        row.final_text = row.raw_text.clone();
        MeetingArchive::polish_segments(&Polish::default(), &mut row);
        assert_eq!(
            row.segments[0].polished_text.as_deref(),
            Some("So the budget is agreed.")
        );
        assert_eq!(
            row.segments[1].text, "we ship on thursday",
            "raw words stay"
        );
        assert_eq!(
            row.final_text,
            "So the budget is agreed. We ship on thursday."
        );
        assert!(row.raw_text.starts_with("um so"));
    }
    #[test]
    fn completion_keeps_the_retention_choice_from_before_postprocessing() {
        use dettivo_meeting::Archive;
        let tmp = tempfile::tempdir().unwrap();
        let paths = dettivo_core::paths::Paths::from_env(|key| match key {
            "HOME" | "XDG_RUNTIME_DIR" => Some(tmp.path().into()),
            _ => None,
        });
        let mut loaded = Loaded::load(&paths.config_file, |_| None);
        loaded.config.meetings.artifacts = "none".into();
        loaded.config.meetings.keep_audio = false;
        loaded.config.meetings.analysis.auto = false;
        let history = Arc::new(History::open(&paths, &loaded).unwrap());
        let config = Arc::new(RwLock::new(loaded));
        let analysis = Arc::new(Analysis::new(
            config.clone(),
            Arc::new(crate::events::EventBus::new()),
            history.clone(),
        ));
        let archive = MeetingArchive::new(history.clone(), analysis, config.clone());
        let mut row = MeetingRow::new();
        row.status = dettivo_storage::meetings::MeetingStatus::Completed;
        row.source_kind = "audioImport".into();
        let dir = history.meeting_artifacts().dir(&row.id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("microphone.wav"), "audio").unwrap();
        row.audio_dir = Some(dir.to_string_lossy().into_owned());
        history.begin_meeting(&row).unwrap();
        archive.set_on_completed(Arc::new(move |row| {
            assert!(
                std::path::Path::new(row.audio_dir.as_ref().unwrap())
                    .join("microphone.wav")
                    .is_file()
            );
            let mut loaded = config.write().unwrap();
            loaded.config.meetings.artifacts = "keep".into();
            loaded.config.meetings.keep_audio = true;
        }));
        archive.settled(&row, false, ArtifactPolicy::None);
        assert!(
            !dir.exists(),
            "postprocessing cannot change the frozen retention choice"
        );
    }
}
