//! The event bus (FR-S5): subscriptions per connection with a bounded
//! buffer, `events.notify` lines pushed to the connection's writer, and
//! `events.overflow` with the drop count when a subscriber falls behind.
//! Publishers are the session, the capture and the engine supervisor.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use dettivo_proto::events::{NOTIFY_METHOD, NotifyParams, OverflowPayload, Topic};
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// A connection's outbound notification channel.
pub type Sink = mpsc::Sender<String>;

struct Subscriber {
    conn: u64,
    topics: Vec<Topic>,
    sink: Sink,
    buffer: u32,
    /// Events dropped since the last overflow notification.
    dropped: Arc<AtomicU64>,
    /// An overflow notification waits for room on the channel.
    overflow_pending: Arc<AtomicBool>,
}

/// The bus.
pub struct EventBus {
    subs: Mutex<BTreeMap<String, Subscriber>>,
    next: AtomicU64,
    /// The runtime that delivers a pending overflow as soon as the
    /// subscriber's channel has room; without one (unit tests) the
    /// overflow rides ahead of the next event that fits.
    runtime: Option<tokio::runtime::Handle>,
    #[cfg(test)]
    after_overflow_count: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    /// An empty bus.
    pub fn new() -> Self {
        Self {
            subs: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
            runtime: tokio::runtime::Handle::try_current().ok(),
            #[cfg(test)]
            after_overflow_count: None,
        }
    }

    /// Subscribes a connection; returns the subscription id.
    pub fn subscribe(&self, conn: u64, topics: Vec<Topic>, buffer: u32, sink: Sink) -> String {
        let id = format!("sub_{}", self.next.fetch_add(1, Ordering::Relaxed));
        self.subs.lock().unwrap_or_else(|p| p.into_inner()).insert(
            id.clone(),
            Subscriber {
                conn,
                topics,
                sink,
                buffer,
                dropped: Arc::new(AtomicU64::new(0)),
                overflow_pending: Arc::new(AtomicBool::new(false)),
            },
        );
        tracing::debug!(subscription = %id, "subscribed");
        id
    }

    /// Removes a subscription; true when it existed on this connection.
    pub fn unsubscribe(&self, conn: u64, id: &str) -> bool {
        let mut g = self.subs.lock().unwrap_or_else(|p| p.into_inner());
        match g.get(id) {
            Some(s) if s.conn == conn => {
                g.remove(id);
                true
            }
            _ => false,
        }
    }

    /// Drops every subscription of a connection that went away.
    pub fn disconnect(&self, conn: u64) {
        self.subs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|_, s| s.conn != conn);
    }

    /// How many subscriptions exist.
    pub fn count(&self) -> usize {
        self.subs.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Publishes `payload` on `topic` to every subscriber of it. A full
    /// channel drops the event and counts it; `events.overflow` with the
    /// count is delivered as soon as the channel has room again (or, with
    /// no runtime, ahead of the next event that fits).
    pub fn publish(&self, topic: Topic, payload: Value) {
        let timestamp = now_iso();
        let mut g = self.subs.lock().unwrap_or_else(|p| p.into_inner());
        for (id, s) in g.iter_mut() {
            if !s.topics.contains(&topic) {
                continue;
            }
            if self.runtime.is_none() && s.dropped.load(Ordering::Relaxed) > 0 {
                let n = s.dropped.load(Ordering::Relaxed);
                let overflow = line(
                    id,
                    Topic::EventsOverflow,
                    &timestamp,
                    json!(OverflowPayload { dropped: n }),
                );
                match s.sink.try_send(overflow) {
                    Ok(()) => {
                        s.dropped.fetch_sub(n, Ordering::Relaxed);
                    }
                    Err(_) => {
                        s.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                }
            }
            let text = line(id, topic, &timestamp, payload.clone());
            if s.sink.try_send(text).is_err() {
                let n = s.dropped.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 {
                    tracing::debug!(subscription = %id, buffer = s.buffer, "subscriber buffer full; dropping");
                }
                if let Some(rt) = &self.runtime {
                    if !s.overflow_pending.swap(true, Ordering::SeqCst) {
                        let (sink, dropped, pending, id) = (
                            s.sink.clone(),
                            s.dropped.clone(),
                            s.overflow_pending.clone(),
                            id.clone(),
                        );
                        #[cfg(test)]
                        let after_count = self.after_overflow_count.clone();
                        rt.spawn(async move {
                            while let Ok(permit) = sink.reserve().await {
                                let n = dropped.swap(0, Ordering::SeqCst);
                                #[cfg(test)]
                                if let Some(after_count) = &after_count {
                                    after_count();
                                }
                                if n > 0 {
                                    permit.send(line(
                                        &id,
                                        Topic::EventsOverflow,
                                        &now_iso(),
                                        json!(OverflowPayload { dropped: n }),
                                    ));
                                }
                                pending.store(false, Ordering::SeqCst);
                                if dropped.load(Ordering::SeqCst) == 0
                                    || pending.swap(true, Ordering::SeqCst)
                                {
                                    return;
                                }
                            }
                            pending.store(false, Ordering::SeqCst);
                        });
                    }
                }
            }
        }
    }
}

fn line(id: &str, topic: Topic, timestamp: &str, payload: Value) -> String {
    let params = NotifyParams {
        subscription_id: id.to_string(),
        topic,
        timestamp: timestamp.to_string(),
        payload,
    };
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "method": NOTIFY_METHOD,
        "params": params,
    }))
    .unwrap_or_default()
}

/// ISO 8601 UTC with milliseconds.
pub(crate) fn now_iso() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let millis = d.subsec_millis();
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, dd) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{dd:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_final_drop_between_count_and_release_is_reported_without_more_publishing() {
        let mut bus = EventBus::new();
        let weak = Arc::new(Mutex::new(std::sync::Weak::<EventBus>::new()));
        let once = Arc::new(AtomicBool::new(false));
        bus.after_overflow_count = Some(Arc::new({
            let weak = weak.clone();
            move || {
                if !once.swap(true, Ordering::SeqCst) {
                    weak.lock()
                        .unwrap()
                        .upgrade()
                        .unwrap()
                        .publish(Topic::DictationState, json!({"last":true}));
                }
            }
        }));
        let bus = Arc::new(bus);
        *weak.lock().unwrap() = Arc::downgrade(&bus);
        let (tx, mut rx) = mpsc::channel(1);
        bus.subscribe(1, vec![Topic::DictationState], 1, tx);
        bus.publish(Topic::DictationState, json!({"first":true}));
        bus.publish(Topic::DictationState, json!({"dropped":true}));
        rx.recv().await.unwrap();
        let mut total = 0;
        while total < 2 {
            let message = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
                .await
                .expect("the final dropped event must be reported even when publishers stop")
                .unwrap();
            let event: Value = serde_json::from_str(&message).unwrap();
            total += event["params"]["payload"]["dropped"].as_u64().unwrap();
        }
        assert_eq!(total, 2);
    }

    #[test]
    fn a_full_buffer_drops_and_reports_the_count_once_space_frees() {
        let bus = EventBus::new();
        let (tx, mut rx) = mpsc::channel::<String>(2);
        let id = bus.subscribe(1, vec![Topic::DictationState], 2, tx);
        for i in 0..5 {
            bus.publish(Topic::DictationState, json!({"n": i}));
        }
        // Two fit, three dropped.
        let first: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(first["params"]["payload"]["n"], 0);
        let _ = rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
        bus.publish(Topic::DictationState, json!({"n": 5}));
        let overflow: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(overflow["params"]["topic"], "events.overflow");
        assert_eq!(overflow["params"]["payload"]["dropped"], 3);
        assert_eq!(overflow["params"]["subscription_id"], id);
        let next: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(next["params"]["payload"]["n"], 5);
        assert!(bus.unsubscribe(1, &id));
        assert!(!bus.unsubscribe(1, &id));
        assert_eq!(bus.count(), 0);
    }

    #[test]
    fn topics_filter_and_disconnect_clears() {
        let bus = EventBus::new();
        let (tx, mut rx) = mpsc::channel::<String>(8);
        bus.subscribe(7, vec![Topic::AudioLevel], 8, tx);
        bus.publish(Topic::DictationState, json!({}));
        assert!(rx.try_recv().is_err());
        bus.publish(Topic::AudioLevel, json!({"rms": 0.1}));
        let v: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(v["method"], "events.notify");
        assert!(v["params"]["timestamp"].as_str().unwrap().ends_with('Z'));
        bus.disconnect(7);
        assert_eq!(bus.count(), 0);
    }

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
    }
}
