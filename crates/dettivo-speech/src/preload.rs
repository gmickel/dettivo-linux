//! Coalesced preloads (FR-E6): `startup`, `model_selection` and
//! `session_start` all ask for the same thing, a warm engine; one load
//! runs at a time, later requests while it runs join it, and a failure is
//! logged privacy-safe and never fails the caller.

use std::sync::{Arc, Condvar, Mutex};

use crate::{PreloadSource, SttEngine};

/// The coalescing gate around an engine's `preload`.
pub struct Preloader {
    engine: Arc<dyn SttEngine>,
    state: Mutex<State>,
    done: Condvar,
}

#[derive(Default)]
struct State {
    running: bool,
    requested: Vec<PreloadSource>,
    attempts: u32,
    successes: u32,
}

impl Preloader {
    /// Wraps `engine`.
    pub fn new(engine: Arc<dyn SttEngine>) -> Arc<Self> {
        Arc::new(Self {
            engine,
            state: Mutex::new(State::default()),
            done: Condvar::new(),
        })
    }

    /// Requests a preload from `source`. Runs it when none is running,
    /// otherwise waits for the running one. Never returns an error.
    pub fn request(&self, source: PreloadSource) {
        let run_it = {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            s.requested.push(source);
            if s.running {
                let mut guard = s;
                while guard.running {
                    guard = self.done.wait(guard).unwrap_or_else(|p| p.into_inner());
                }
                return;
            }
            s.running = true;
            s.attempts += 1;
            true
        };
        if run_it {
            let result = self.engine.preload(source);
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            match result {
                Ok(()) => s.successes += 1,
                Err(e) => {
                    tracing::warn!(source = ?source, error = %e, "preload failed (non-fatal)")
                }
            }
            // The sources that joined this attempt are only diagnostics for
            // the attempt itself; drop them so the list stays bounded.
            s.requested.clear();
            s.running = false;
            self.done.notify_all();
        }
    }

    /// `(attempts, successes)` so far.
    pub fn stats(&self) -> (u32, u32) {
        let s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        (s.attempts, s.successes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Capabilities, EngineError, RecognizeRequest};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    struct Slow(AtomicU32, bool);

    impl SttEngine for Slow {
        fn provider(&self) -> &str {
            "slow"
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_timestamps: true,
                supports_streaming: false,
                supported_languages: vec![],
                max_duration_seconds: 1,
                supports_custom_vocabulary: false,
                supports_model_deletion: false,
            }
        }
        fn preload(&self, _source: PreloadSource) -> Result<(), EngineError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(150));
            if self.1 {
                Ok(())
            } else {
                Err(EngineError::ModelMissing("nope".into()))
            }
        }
        fn recognize(
            &self,
            _request: RecognizeRequest,
            _timeout: Duration,
        ) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
            unreachable!()
        }
        fn cancel(&self) {}
        fn backend(&self) -> Option<dettivo_engine_proto::Backend> {
            None
        }
    }

    #[test]
    fn concurrent_sources_coalesce_into_one_load() {
        let engine = Arc::new(Slow(AtomicU32::new(0), true));
        let preloader = Preloader::new(engine.clone());
        let handles: Vec<_> = [
            PreloadSource::Startup,
            PreloadSource::ModelSelection,
            PreloadSource::SessionStart,
        ]
        .into_iter()
        .map(|source| {
            let p = preloader.clone();
            std::thread::spawn(move || p.request(source))
        })
        .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            engine.0.load(Ordering::SeqCst),
            1,
            "one load served three sources"
        );
        assert_eq!(preloader.stats(), (1, 1));
    }

    #[test]
    fn a_failed_preload_never_fails_the_caller() {
        let engine = Arc::new(Slow(AtomicU32::new(0), false));
        let preloader = Preloader::new(engine);
        preloader.request(PreloadSource::Startup);
        assert_eq!(preloader.stats(), (1, 0));
    }
}
