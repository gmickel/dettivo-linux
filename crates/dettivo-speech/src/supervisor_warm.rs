//! The warm-up after a load (ADR 0084): a freshly loaded speech engine
//! compiles its GPU kernels on its first inference, so a preload runs one
//! silent `recognize` before the first real take. It is skipped when the
//! engine recognised within `WARM_UP_SKIP` or a real request is waiting;
//! a real request that asked first runs first and warms the engine itself.

use std::time::{Duration, Instant};

use super::{EngineTransition, Slot, Supervisor};
use crate::EngineError;
use crate::process::EngineProcess;

/// No warm-up when the engine recognised this recently.
pub const WARM_UP_SKIP: Duration = Duration::from_secs(60);

/// The warm-up's silence: 30 s at 16 kHz, the longest window Whisper
/// decodes at once; a shorter one leaves Parakeet's long-input kernels
/// to the first take (ADR 0084).
pub const WARM_UP_SAMPLES: usize = 480_000;

/// A real request on one engine, counted from the moment it asks until it
/// finishes; a warm-up does not start while one is counted.
pub struct RequestGuard<'a> {
    supervisor: &'a Supervisor,
    binary: String,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        let mut pending = self
            .supervisor
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(n) = pending.get_mut(&self.binary) {
            *n = n.saturating_sub(1);
        }
    }
}

impl Supervisor {
    /// Counts a real request on `binary` until the guard drops.
    pub fn begin_request(&self, binary: &str) -> RequestGuard<'_> {
        *self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(binary.to_string())
            .or_default() += 1;
        RequestGuard {
            supervisor: self,
            binary: binary.to_string(),
        }
    }

    fn waiting(&self, binary: &str) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(binary)
            .is_some_and(|n| *n > 0)
    }

    /// Runs `run` (the silent `recognize`) against `binary`'s loaded model
    /// unless it is already warm, recognised within `WARM_UP_SKIP`, has no
    /// model loaded, or a real request is waiting. Never loads or spawns.
    /// Returns how long the warm-up took, `None` when it was skipped.
    pub fn warm_up(
        &self,
        binary: &str,
        run: impl FnOnce(&mut EngineProcess) -> Result<(), EngineError>,
    ) -> Result<Option<Duration>, EngineError> {
        let settings = self.settings();
        let slot = self.slot(binary);
        let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
        let recent = s
            .last_recognized
            .is_some_and(|t| t.elapsed() < WARM_UP_SKIP);
        let alive = s.process.as_mut().is_some_and(EngineProcess::alive);
        let params = s
            .params
            .clone()
            .filter(|_| alive && s.loaded.is_some() && !s.warm && !recent);
        let Some(params) = params else {
            return Ok(None);
        };
        if self.waiting(binary) {
            return Ok(None);
        }
        let started = Instant::now();
        // The loaded parameters themselves, so `with_locked` never reloads.
        let result = self.with_locked((binary, binary), params, &settings, &mut s, |p, _| run(p));
        let took = started.elapsed();
        let outcome = result.map(|()| {
            self.mark_warm(&mut s, binary);
            Some(took)
        });
        self.remember(binary, &mut s);
        outcome
    }

    /// Records a real recognition of `model` on `binary`: no warm-up runs
    /// within `WARM_UP_SKIP`, and the loaded model is warm from now on when
    /// it is still `model` (a selection may have loaded another meanwhile).
    pub fn note_recognized(&self, binary: &str, model: &str) {
        let slot = self.slot(binary);
        let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
        s.last_recognized = Some(Instant::now());
        if s.params.as_ref().is_some_and(|p| p.model == model) {
            self.mark_warm(&mut s, binary);
        }
    }

    fn mark_warm(&self, s: &mut Slot, binary: &str) {
        if s.warm {
            return;
        }
        let Some(loaded) = s.loaded.as_ref() else {
            return;
        };
        s.warm = true;
        let transition = EngineTransition {
            binary: binary.to_string(),
            state: "warm",
            model: Some(loaded.model.clone()),
            backend: Some(loaded.backend),
            reason: None,
        };
        self.transition(s, transition);
    }
}
