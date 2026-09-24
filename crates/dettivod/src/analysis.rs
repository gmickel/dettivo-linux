//! The meeting analysis service (ADR 0036): one job per meeting that runs
//! the finalised transcript through the language model provider layer
//! (`[meetings.analysis] provider`, empty follows `[llm] provider`; the
//! local engine bound to `[llm] analysis_model`), stores the summary,
//! decisions and action items on the row and in `analysis.json`, and
//! reports where it stands through `meeting.state` with
//! `analysis_status`. It starts on its own after a finalisation when
//! `[meetings.analysis] auto` is on (or the meeting asked for it), and on
//! `meetings.analyze`. A run never touches the notes; a failed run keeps
//! the previous analysis and records why. The run commits its result
//! through the meeting's job entry (`meeting_jobs`), so a delete that
//! invalidated it drops the result instead of restoring the old analysis.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use dettivo_core::config::Loaded;
use dettivo_core::config::llm_schema::LlmProviderChoice;
use dettivo_language::analysis::{self, Failure, Settings};
use dettivo_language::provider::{self, LlmProvider, LocalEngine};
use dettivo_proto::events::{MeetingLiveState, MeetingStatePayload, Topic};
use dettivo_proto::id::Id;
use dettivo_proto::methods::meetings_notes::AnalysisStatus;
use dettivo_storage::meeting_notes;
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use serde_json::json;

use crate::events::EventBus;
use crate::history::History;
use crate::meeting_jobs::{Job, MeetingJobs};

/// One analysis in flight (`job_analysis_<n>`).
pub type Running = Job;

/// Why a run was not started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A run is in flight for this meeting; the job id.
    Running(String),
    /// The meeting already has an analysis and `force` was not asked.
    AlreadyReady,
    /// The meeting has no final transcript; its status.
    NotCompleted(MeetingStatus),
    /// The configured endpoint is remote and not trusted.
    EndpointNotTrusted(String),
    /// No provider answers; what would fix it.
    NoProvider {
        /// One line for a person.
        detail: String,
        /// What to do about it.
        hint: Option<String>,
    },
}

/// The service.
pub struct Analysis {
    config: Arc<RwLock<Loaded>>,
    bus: Arc<EventBus>,
    history: Arc<History>,
    local: Mutex<Option<Arc<dyn LocalEngine>>>,
    jobs: MeetingJobs,
    overrides: Mutex<std::collections::HashMap<String, bool>>,
    seq: AtomicU64,
}

impl Analysis {
    /// A service over the configuration cell, the bus and the store.
    pub fn new(config: Arc<RwLock<Loaded>>, bus: Arc<EventBus>, history: Arc<History>) -> Self {
        Self {
            config,
            bus,
            history,
            local: Mutex::new(None),
            jobs: MeetingJobs::default(),
            overrides: Mutex::new(std::collections::HashMap::new()),
            seq: AtomicU64::new(0),
        }
    }

    /// Attaches the local engine bound to the analysis model.
    pub fn set_local_llm(&self, engine: Arc<dyn LocalEngine>) {
        *self.local.lock().unwrap_or_else(|p| p.into_inner()) = Some(engine);
    }

    /// Remembers what a meeting asked for at start (`analyze`), read once
    /// when it settles.
    pub fn set_override(&self, meeting_id: &str, analyze: bool) {
        self.overrides
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(meeting_id.to_string(), analyze);
    }

    /// Whether a meeting that just settled is analysed: its own choice,
    /// else `[meetings.analysis] auto`.
    pub fn wants_auto(&self, meeting_id: &str) -> bool {
        let chosen = self
            .overrides
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(meeting_id);
        chosen.unwrap_or_else(|| {
            self.config
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .config
                .meetings
                .analysis
                .auto
        })
    }

    /// The run in flight for `meeting_id`, when any.
    pub fn running(&self, meeting_id: &str) -> Option<Running> {
        self.jobs.get(meeting_id)
    }

    /// Stops the run for `meeting_id`; true when one ran. Its result is
    /// dropped and the row is left as it was; a commit in progress
    /// finishes first, so a delete that follows clears after it.
    pub fn cancel(&self, meeting_id: &str) -> bool {
        self.jobs.invalidate(meeting_id)
    }

    /// The provider the analysis asks, resolved from the configuration.
    fn provider(&self) -> Option<Box<dyn LlmProvider>> {
        let loaded = self.config.read().unwrap_or_else(|p| p.into_inner());
        let mut llm = loaded.config.llm.clone();
        match loaded.config.meetings.analysis.provider.trim() {
            "auto" => llm.provider = LlmProviderChoice::Auto,
            "local" => llm.provider = LlmProviderChoice::Local,
            "ollama" => llm.provider = LlmProviderChoice::Ollama,
            "openai_compatible" => llm.provider = LlmProviderChoice::OpenaiCompatible,
            _ => {}
        }
        let local = self.local.lock().unwrap_or_else(|p| p.into_inner()).clone();
        provider::resolve(&llm, local.as_ref())
    }

    fn settings(&self) -> Settings {
        let loaded = self.config.read().unwrap_or_else(|p| p.into_inner());
        let a = &loaded.config.meetings.analysis;
        Settings {
            chunk_chars: usize::try_from(a.chunk_chars).unwrap_or(12_000).max(200),
            timeout: Duration::from_millis(a.timeout_ms.max(1)),
        }
    }

    fn publish(&self, row: &MeetingRow, status: AnalysisStatus, reason: Option<String>) {
        let Ok(meeting_id) = Id::new(row.id.clone()) else {
            return;
        };
        self.bus.publish(
            Topic::MeetingState,
            json!(MeetingStatePayload {
                kind: "meeting".into(),
                state: MeetingLiveState::Completed,
                meeting_id,
                live_segment_count: row.segments.len() as u64,
                live_last_end_ms: row.segments.last().map(|s| s.end_ms).unwrap_or(0),
                is_finalizing: false,
                previous_state: Some(MeetingLiveState::Completed),
                reason,
                duration_ms: Some(row.duration_ms),
                microphone_takes: Some(row.microphone_takes),
                system_audio: Some(row.system_audio),
                diarization_status: None,
                analysis_status: Some(status),
            }),
        );
    }

    /// Marks the row failed with `code: message` and says so on the bus.
    fn fail(&self, row: &MeetingRow, code: &str, message: &str) {
        let error = format!("{code}: {message}");
        if let Err(e) = self.history.with_store(|s| {
            s.set_meeting_analysis_status(&row.id, AnalysisStatus::Failed, Some(&error))
        }) {
            tracing::warn!(error = %e, "analysis: failed status not stored");
        }
        tracing::warn!(id = %row.id, code, "meeting analysis failed");
        self.publish(row, AnalysisStatus::Failed, Some(error));
    }

    /// Settles an analysis the finalisation planned (`queued` on the row)
    /// that `start` refused without marking the row itself: the row
    /// becomes `failed` with the refusal as the reason, so a planned
    /// analysis never stays queued for good. A refusal that already
    /// failed the row (no provider) leaves it as it is.
    pub(crate) fn settle_queued(&self, meeting_id: &str, refusal: &Refusal) {
        let Ok(Some(current)) = self.history.with_store(|s| s.get_meeting(meeting_id)) else {
            return;
        };
        if current.analysis_status != AnalysisStatus::Queued {
            return;
        }
        let (code, message) = match refusal {
            Refusal::Running(job) => (
                "analysisRunning",
                format!("an analysis runs already ({job})"),
            ),
            Refusal::AlreadyReady => (
                "alreadyReady",
                "the meeting already has an analysis".to_string(),
            ),
            Refusal::NotCompleted(status) => (
                "meetingNotCompleted",
                format!("the meeting is {}", status.as_str()),
            ),
            Refusal::EndpointNotTrusted(detail) => ("endpointNotTrusted", detail.clone()),
            Refusal::NoProvider { detail, hint } => (
                "provider_unavailable",
                match hint {
                    Some(h) => format!("{detail}; {h}"),
                    None => detail.clone(),
                },
            ),
        };
        self.fail(&current, code, &message);
    }

    /// Starts the analysis of `row` on a thread of its own; `force` runs
    /// again over a meeting that already has one.
    pub fn start(self: &Arc<Self>, row: MeetingRow, force: bool) -> Result<Running, Refusal> {
        let keep_artifacts = self
            .config
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .config
            .meetings
            .artifacts
            == "keep";
        self.start_with_artifacts(row, force, keep_artifacts)
    }

    pub(crate) fn start_with_artifacts(
        self: &Arc<Self>,
        row: MeetingRow,
        force: bool,
        keep_artifacts: bool,
    ) -> Result<Running, Refusal> {
        if let Some(r) = self.running(&row.id) {
            return Err(Refusal::Running(r.job_id));
        }
        if row.status != MeetingStatus::Completed {
            return Err(Refusal::NotCompleted(row.status));
        }
        if row.analysis_status == AnalysisStatus::Ready && !force {
            return Err(Refusal::AlreadyReady);
        }
        let provider = match self.provider() {
            Some(p) => p,
            None => {
                self.fail(&row, "provider_unavailable", "no language model provider is available; see [llm] and [meetings.analysis] in config.toml");
                return Err(Refusal::NoProvider {
                    detail: "no language model provider is available".into(),
                    hint: Some("see [llm] in config.toml or run dettivo llm download".into()),
                });
            }
        };
        let availability = provider.probe();
        if !availability.available {
            if availability.detail.contains("not a trusted endpoint") {
                return Err(Refusal::EndpointNotTrusted(availability.detail));
            }
            self.fail(&row, "provider_unavailable", &availability.detail);
            return Err(Refusal::NoProvider {
                detail: availability.detail,
                hint: availability.hint,
            });
        }
        let n = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let running = Job::new(format!("job_analysis_{n}"));
        self.jobs.insert(&row.id, running.clone());
        if let Err(e) = self
            .history
            .with_store(|s| s.set_meeting_analysis_status(&row.id, AnalysisStatus::Running, None))
        {
            tracing::warn!(error = %e, "analysis: running status not stored");
        }
        self.publish(&row, AnalysisStatus::Running, None);
        tracing::info!(id = %row.id, job = %running.job_id, provider = provider.name(), "meeting analysis started");
        let me = self.clone();
        let job = running.clone();
        let settings = self.settings();
        let meeting_id = row.id.clone();
        std::thread::Builder::new()
            .name(running.job_id.clone())
            .spawn(move || me.run(row, provider, settings, job, keep_artifacts))
            .map_err(|e| {
                self.jobs.remove(&meeting_id, &running.job_id);
                Refusal::NoProvider {
                    detail: format!("cannot start the analysis thread: {e}"),
                    hint: None,
                }
            })?;
        Ok(running)
    }

    fn run(
        &self,
        row: MeetingRow,
        provider: Box<dyn LlmProvider>,
        settings: Settings,
        job: Running,
        keep_artifacts: bool,
    ) {
        let transcript = if row.segments.is_empty() {
            row.final_text.clone()
        } else {
            analysis::transcript_lines(&row.segments)
        };
        let outcome = analysis::run(&transcript, provider.as_ref(), &settings, Some(&job.cancel));
        // The result is written only by the job that still owns the
        // meeting, under the lock a cancel or a delete takes to
        // invalidate it, so a delete never sees the old analysis return.
        let committed = self.jobs.commit(&row.id, &job.job_id, || {
            if job.cancel.load(Ordering::Relaxed) {
                return false;
            }
            match outcome {
                Ok(out) => {
                    let at = dettivo_storage::time::now_iso();
                    if let Err(e) = self.history.with_store(|s| {
                        s.set_meeting_analysis(&row.id, &out.analysis, &out.model, &at)
                    }) {
                        tracing::warn!(error = %e, "analysis: result not stored");
                        return true;
                    }
                    let dir = self.history.meeting_artifacts().dir(&row.id);
                    if keep_artifacts && let Err(e) =
                        meeting_notes::write_analysis_file(&dir, &out.analysis, &out.model, &at)
                    {
                        tracing::debug!(error = %e, "analysis: analysis.json not written");
                    }
                    tracing::info!(id = %row.id, model = %out.model, calls = out.calls, parts = out.parts, "meeting analysis ready");
                    self.publish(&row, AnalysisStatus::Ready, None);
                }
                Err(Failure::Cancelled) => {}
                Err(e) => self.fail(&row, e.code(), &e.to_string()),
            }
            true
        });
        if committed != Some(true) {
            tracing::info!(id = %row.id, "meeting analysis cancelled; result dropped");
        }
    }
}
