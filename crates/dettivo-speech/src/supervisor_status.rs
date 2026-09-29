//! Status snapshots remain readable while an engine's request lock is held.

use dettivo_engine_proto::Backend;

use super::{EngineStatus, EngineTransition, Slot, Supervisor};
use crate::process::{EngineProcess, find_binary};

impl Supervisor {
    /// Records that a loaded engine moved itself to `backend` (the
    /// language model's in-process CPU fallback), so status, doctor and
    /// the tier stop reporting the backend it was loaded on.
    pub fn note_backend(&self, binary: &str, backend: Backend, reason: &str) {
        let slot = self.slot(binary);
        let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
        let Some(loaded) = s.loaded.as_mut().filter(|l| l.backend != backend) else {
            return;
        };
        loaded.backend = backend;
        loaded.reason = format!("{}; {reason}", loaded.reason);
        let transition = EngineTransition {
            binary: binary.to_string(),
            state: "loaded",
            model: Some(loaded.model.clone()),
            backend: Some(backend),
            reason: Some(loaded.reason.clone()),
        };
        self.transition(&mut s, transition);
    }

    pub(super) fn remember(&self, binary: &str, slot: &mut Slot) -> EngineStatus {
        let running = slot.process.as_mut().is_some_and(EngineProcess::alive);
        let status = EngineStatus {
            binary: binary.to_string(),
            path: slot.process.as_ref().map(|p| p.binary.clone()),
            running,
            model: slot.loaded.as_ref().map(|l| l.model.clone()),
            backend: slot.loaded.as_ref().map(|l| l.backend),
            reason: slot.loaded.as_ref().map(|l| l.reason.clone()),
            lora: slot.params.as_ref().and_then(|p| p.lora.clone()),
            crashes: slot.crashes,
            degraded: slot.crashes >= 3,
            memory_bytes: running
                .then(|| slot.process.as_ref().and_then(EngineProcess::memory_bytes))
                .flatten(),
        };
        self.snapshots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(binary.to_string(), status.clone());
        status
    }

    /// Status per known engine, using the last transition while it is busy.
    pub fn status(&self, binaries: &[&str]) -> Vec<EngineStatus> {
        let settings = self.settings();
        binaries
            .iter()
            .map(|binary| {
                let path = find_binary(binary, settings.directory.as_deref()).ok();
                let slot = self
                    .slots
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(*binary)
                    .cloned();
                let snapshot = slot.and_then(|slot| {
                    if let Ok(mut s) = slot.try_lock() {
                        Some(self.remember(binary, &mut s))
                    } else {
                        self.snapshots
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .get(*binary)
                            .cloned()
                    }
                });
                let mut status = snapshot.unwrap_or_else(|| EngineStatus {
                    binary: binary.to_string(),
                    path: None,
                    running: false,
                    model: None,
                    backend: None,
                    reason: None,
                    lora: None,
                    crashes: 0,
                    degraded: false,
                    memory_bytes: None,
                });
                status.path = path;
                status
            })
            .collect()
    }
}
