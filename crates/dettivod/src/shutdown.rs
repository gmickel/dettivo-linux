//! One deadline for the whole shutdown: `[daemon] shutdown_timeout_ms`
//! starts at the signal and bounds the connection drain, the listener
//! release and every service stop after it. A step that does not finish
//! by the deadline is left behind and named in the log; the daemon exits
//! anyway, and an engine child ends on the closed pipe.

use std::time::{Duration, Instant};

/// Runs `step` on a thread of its own and waits for it until `deadline`;
/// true when it finished in time. A step still running at the deadline is
/// abandoned with a warning naming it.
pub fn within(deadline: Instant, name: &'static str, step: impl FnOnce() + Send + 'static) -> bool {
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let spawned = std::thread::Builder::new()
        .name(format!("shutdown-{name}"))
        .spawn(move || {
            step();
            let _ = done_tx.send(());
        });
    if spawned.is_err() {
        tracing::warn!(step = name, "shutdown step could not start; skipped");
        return false;
    }
    let budget = deadline.saturating_duration_since(Instant::now());
    match done_rx.recv_timeout(budget.max(Duration::from_millis(1))) {
        Ok(()) => true,
        Err(_) => {
            tracing::warn!(step = name, "shutdown budget exhausted; step left behind");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// daemon/F10: a step that stalls does not hold the shutdown past the
    /// deadline; a quick one finishes inside it.
    #[test]
    fn a_stalled_step_is_left_behind_at_the_deadline() {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(150);
        assert!(!within(deadline, "stalled", || {
            std::thread::sleep(Duration::from_secs(10));
        }));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(within(
            Instant::now() + Duration::from_secs(5),
            "quick",
            || {}
        ));
        // A deadline already past still lets nothing block.
        let past = Instant::now() - Duration::from_secs(1);
        assert!(!within(past, "late", || std::thread::sleep(
            Duration::from_secs(10)
        )));
    }
}
