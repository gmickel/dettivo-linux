//! dictation/F1: a cancel that lands while the language pipeline rewrites
//! the take stops the session before anything is typed or archived, and a
//! cancel that arrives after the delivery says the session is gone rather
//! than reporting a cancellation that did not happen.

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use dettivo_language::provider::local::LocalEngine;
use dettivo_language::provider::{Answer, Availability, ProviderError, RewriteRequest};
use dettivo_session::{SessionError, State};
use support::{policy, rig};

/// A local model that waits for the test at `entered`, then at `release`.
struct BlockingModel {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: AtomicUsize,
}

impl LocalEngine for BlockingModel {
    fn model(&self) -> String {
        "fake".into()
    }
    fn probe(&self) -> Availability {
        Availability {
            available: true,
            detail: "fake".into(),
            hint: None,
        }
    }
    fn generate(&self, _: &RewriteRequest, _: Duration) -> Result<Answer, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.wait();
        self.release.wait();
        Ok(Answer::unknown("Rewritten by the model.".into()))
    }
}

#[test]
fn a_cancel_during_the_rewrite_inserts_and_archives_nothing() {
    let (dir, rec, session, engine, feed) = rig("the take as heard");
    let model = Arc::new(BlockingModel {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        calls: AtomicUsize::new(0),
    });
    session.set_local_llm(Some(model.clone()));
    let mut policy = policy(dir.path(), false);
    policy.mode = "enhanced".into();
    policy.llm.provider = dettivo_core::config::llm_schema::LlmProviderChoice::Local;
    policy.llm.timeout_ms = 30_000;
    session.start(policy, engine, feed.clone(), None).unwrap();
    feed.speak(100);
    let session = Arc::new(session);
    let stopper = {
        let s = session.clone();
        std::thread::spawn(move || s.stop())
    };
    // The worker is inside the model call now, between transcription and
    // insertion: the window the cancel must cover.
    model.entered.wait();
    let canceller = {
        let s = session.clone();
        std::thread::spawn(move || s.cancel())
    };
    std::thread::sleep(Duration::from_millis(100));
    model.release.wait();
    let cancelled = canceller.join().unwrap().unwrap();
    assert_eq!(cancelled.state, State::Cancelled);
    assert_eq!(
        stopper.join().unwrap().unwrap_err(),
        SessionError::Cancelled
    );
    assert!(rec.inserted.lock().unwrap().is_empty(), "nothing was typed");
    assert!(session.last().is_none(), "nothing was kept");
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        rec.sequence(),
        [
            State::Recording,
            State::Transcribing,
            State::Cancelled,
            State::Idle
        ]
    );
}
