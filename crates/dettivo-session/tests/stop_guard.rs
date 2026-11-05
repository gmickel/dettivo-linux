//! A delayed stop belongs to the take selected by the caller.
mod support;
use dettivo_session::{SessionError, State};
use std::time::Instant;
use support::{Feed, policy, rig};

#[test]
fn a_delayed_stop_never_stops_a_replacement_take() {
    let (dir, _rec, session, engine, feed) = rig("hello");
    let first = session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed,
            None,
            Instant::now(),
        )
        .unwrap();
    session.cancel().unwrap();
    let second = session
        .start(
            policy(dir.path(), false),
            engine,
            Feed::new(),
            None,
            Instant::now(),
        )
        .unwrap();
    assert_eq!(
        session
            .stop_expected(Some(&first.job_id), Instant::now())
            .unwrap_err(),
        SessionError::NoSession
    );
    let active = session.snapshot().unwrap();
    assert_eq!(active.job_id, second.job_id);
    assert_eq!(active.state, State::Recording);
    session.cancel().unwrap();
}
