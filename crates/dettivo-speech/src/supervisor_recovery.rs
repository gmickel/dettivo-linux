//! How the supervisor recovers an engine: the crash count and its restart
//! backoff, and the CPU fallback after the GPU failed together with its
//! way back. A fallback holds for `Settings::gpu_retry_hold` (two minutes);
//! the first request after that respawns the engine on the GPU. A retry
//! that fails the same way falls back again and doubles the hold, up to
//! `GPU_RETRY_CAP`; a request served on the GPU starts the hold over.

use std::time::{Duration, Instant};

use dettivo_engine_proto::LoadParams;

use super::{EngineTransition, Slot};

/// The longest a fallback holds before the GPU is tried again.
pub const GPU_RETRY_CAP: Duration = Duration::from_secs(30 * 60);

/// An automatic load kept on the CPU because the GPU failed it.
#[derive(Debug, Default)]
pub(super) struct Fallback {
    /// The load that runs on the CPU instead.
    params: Option<LoadParams>,
    /// The current hold; zero until the GPU fails for the first time.
    hold: Duration,
    /// When the next request may try the GPU again.
    retry_at: Option<Instant>,
}

/// Where a request's load goes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Route {
    /// As asked: no fallback applies.
    Asked,
    /// On the CPU: the fallback holds.
    Cpu,
    /// On the GPU again, in a fresh process: the hold has passed.
    RetryGpu,
}

impl Fallback {
    /// Marks `params` for the CPU after the GPU failed it. The hold is
    /// `first` the first time and doubles on every repeat, up to the cap.
    pub fn fall_back(&mut self, params: LoadParams, first: Duration) {
        self.hold = if self.hold.is_zero() {
            first
        } else {
            (self.hold * 2).min(GPU_RETRY_CAP.max(first))
        };
        self.retry_at = Some(Instant::now() + self.hold);
        self.params = Some(params);
    }

    /// Whether a load runs on the CPU because of this fallback.
    pub fn active(&self) -> bool {
        self.params.is_some()
    }

    /// How `params` loads now. Once the hold has passed the fallback
    /// lifts, keeping the hold so a repeat failure doubles it; a changed
    /// load forgets the fallback altogether.
    pub fn route(&mut self, params: &LoadParams) -> Route {
        match &self.params {
            Some(held) if held == params => {
                if self.retry_at.is_some_and(|at| Instant::now() < at) {
                    Route::Cpu
                } else {
                    self.params = None;
                    Route::RetryGpu
                }
            }
            Some(_) => {
                self.clear();
                Route::Asked
            }
            None => Route::Asked,
        }
    }

    /// The engine served a request on the GPU: a later failure starts
    /// from the first hold again.
    pub fn gpu_worked(&mut self) {
        self.hold = Duration::ZERO;
        self.retry_at = None;
    }

    /// Forgets the fallback and its hold (an unload).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    #[cfg(test)]
    fn hold(&self) -> Duration {
        self.hold
    }
}

/// Counts a crash; `tail` is the redacted end of the engine's stderr, the
/// only record of an engine's own abort message (ggml's among them).
pub(super) fn record_crash(s: &mut Slot, binary: &str, tail: &str) -> EngineTransition {
    s.crashes += 1;
    s.process = None;
    s.loaded = None;
    s.params = None;
    let backoff = Duration::from_secs(1 << (s.crashes.min(4) - 1));
    s.next_allowed = Instant::now() + backoff;
    if s.crashes >= 3 {
        tracing::error!(
            engine = binary,
            crashes = s.crashes,
            stderr = %crate::process::diagnostic_lines(tail),
            "engine degraded after repeated crashes"
        );
        EngineTransition {
            binary: binary.to_string(),
            state: "degraded",
            model: None,
            backend: None,
            reason: Some(format!("{} crashes in a row", s.crashes)),
        }
    } else {
        tracing::warn!(
            engine = binary,
            crashes = s.crashes,
            backoff_seconds = backoff.as_secs(),
            stderr = %crate::process::diagnostic_lines(tail),
            "engine crashed"
        );
        EngineTransition {
            binary: binary.to_string(),
            state: "crashed",
            model: None,
            backend: None,
            reason: Some(format!("restart in {} s", backoff.as_secs())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_engine_proto::BackendPreference;

    fn params(model: &str) -> LoadParams {
        LoadParams {
            model: model.into(),
            backend_preference: BackendPreference::Auto,
            vad_model: None,
            context_length: None,
            lora: None,
            threads: None,
        }
    }

    /// The hold doubles on every repeat failure up to thirty minutes, and
    /// a request served on the GPU starts it over.
    #[test]
    fn the_hold_doubles_to_the_cap_and_starts_over_after_the_gpu_works() {
        let first = Duration::from_secs(120);
        let mut f = Fallback::default();
        let mut holds = Vec::new();
        for _ in 0..6 {
            f.fall_back(params("m"), first);
            holds.push(f.hold().as_secs() / 60);
        }
        assert_eq!(holds, [2, 4, 8, 16, 30, 30]);
        assert_eq!(f.route(&params("m")), Route::Cpu);
        f.gpu_worked();
        f.fall_back(params("m"), first);
        assert_eq!(f.hold(), first);
    }

    /// A passed hold sends the next load to the GPU once; a changed load
    /// forgets the fallback.
    #[test]
    fn a_passed_hold_retries_the_gpu_and_a_new_load_forgets_the_fallback() {
        let mut f = Fallback::default();
        f.fall_back(params("m"), Duration::ZERO);
        assert!(f.active());
        assert_eq!(f.route(&params("m")), Route::RetryGpu);
        assert!(!f.active());
        assert_eq!(f.route(&params("m")), Route::Asked);
        f.fall_back(params("m"), Duration::from_secs(60));
        assert_eq!(f.route(&params("other")), Route::Asked);
        assert!(!f.active());
        assert_eq!(f.hold(), Duration::ZERO);
    }
}
