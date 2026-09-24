//! The live side of the meeting scenario (fn-28 R2): the notifications
//! read while the meeting runs, each stamped with its arrival on the
//! scenario clock and whether it landed before the stop was issued, so
//! provisional text is proven delivered to a consumer during the meeting
//! and not merely ordered in a sequence read afterwards.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::daemon::EventStream;

/// How long the meeting runs before the stop: the clip and a little
/// more for the last segments to harden.
pub const PLAY_FOR: Duration = Duration::from_millis(12_500);

/// `params` with the arrival stamp the evidence keeps: `received_ms` on
/// the scenario clock and whether it arrived before the stop was issued.
pub fn stamped(mut params: Value, received_ms: u64, before_stop: bool) -> Value {
    if let Some(map) = params.as_object_mut() {
        map.insert("received_ms".into(), json!(received_ms));
        map.insert("received_before_stop".into(), json!(before_stop));
    }
    params
}

/// Reads the notifications that arrive while the meeting runs, until
/// `window` has passed since `started_at`, each stamped as it lands.
pub fn collect_live(events: &mut EventStream, started_at: Instant, window: Duration) -> Vec<Value> {
    let mut out = Vec::new();
    while let Some(left) = window.checked_sub(started_at.elapsed()) {
        // A quiet second ends a read; the loop runs to the window's end.
        if events.set_read_timeout(Duration::from_secs(1)).is_err() {
            break;
        }
        let mut stamps = Vec::new();
        let batch = events.collect(
            |_| {
                stamps.push(started_at.elapsed().as_millis() as u64);
                false
            },
            left,
        );
        out.extend(
            batch
                .into_iter()
                .zip(stamps)
                .map(|(p, ms)| stamped(p, ms, true)),
        );
    }
    out
}

/// The sources whose provisional segment arrived before the stop.
pub fn provisional_sources(seen: &[Value]) -> Vec<String> {
    let mut sources: Vec<String> = seen
        .iter()
        .filter(|p| {
            p["received_before_stop"] == true
                && p["topic"] == "meeting.segment"
                && p["payload"]["provisional"] == true
        })
        .filter_map(|p| p["payload"]["source"].as_str().map(str::to_string))
        .collect();
    sources.sort();
    sources.dedup();
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(source: &str, provisional: bool, before_stop: bool) -> Value {
        stamped(
            json!({"topic": "meeting.segment", "payload": {"source": source, "provisional": provisional}}),
            if before_stop { 3_000 } else { 14_000 },
            before_stop,
        )
    }

    #[test]
    fn provisional_text_counts_only_when_it_arrived_before_the_stop() {
        let live = vec![
            segment("you", true, true),
            segment("remote", true, true),
            segment("you", false, false),
        ];
        assert_eq!(provisional_sources(&live), ["remote", "you"]);
        assert_eq!(live[0]["received_ms"], 3_000);
        // A daemon that releases every notification after the stop: the
        // same provisional-then-final order, nothing delivered live.
        let held = vec![
            segment("you", true, false),
            segment("remote", true, false),
            segment("you", false, false),
        ];
        assert!(provisional_sources(&held).is_empty());
    }
}
