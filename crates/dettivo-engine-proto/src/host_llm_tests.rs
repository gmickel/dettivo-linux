use super::*;
use crate::messages::{Backend, FinishReason};
use std::sync::mpsc::Sender;

/// Answers with the user text one character per token, so a cancel
/// lands between two tokens.
struct Fake {
    model: String,
}

static LIMITED_RESIDENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Drop for Fake {
    fn drop(&mut self) {
        if self.model.starts_with("limited") {
            LIMITED_RESIDENT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl LanguageEngine for Fake {
    fn load(params: &LoadParams, _force_cpu: bool) -> Result<Self, EngineError> {
        if params.model == "missing" {
            return Err(EngineError::new("model_missing", "no such model"));
        }
        if params.model.starts_with("limited")
            && LIMITED_RESIDENT
                .compare_exchange(
                    0,
                    1,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_err()
        {
            return Err(EngineError::new("out_of_memory", "only one model fits"));
        }
        Ok(Self {
            model: params.model.clone(),
        })
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.model.clone(),
            backend: Backend::Cpu,
            reason: "fake".into(),
            fallback_reason: None,
        }
    }

    fn memory_bytes(&self) -> Option<u64> {
        Some(7)
    }

    fn generate(
        &mut self,
        params: &GenerateParams,
        partial: &mut dyn FnMut(&str),
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<GenerateResult, EngineError> {
        let mut text = String::new();
        let mut tokens = 0;
        for c in params.user.chars() {
            if cancelled() {
                return Ok(GenerateResult {
                    text,
                    tokens,
                    finish_reason: FinishReason::Cancelled,
                    backend: Backend::Cpu,
                });
            }
            text.push(c);
            tokens += 1;
            partial(&c.to_string());
        }
        Ok(GenerateResult {
            text,
            tokens,
            finish_reason: FinishReason::Stop,
            backend: Backend::Cpu,
        })
    }
}

fn rig() -> (Host<Fake, Vec<u8>>, Sender<Incoming>, Receiver<Incoming>) {
    let (tx, rx) = mpsc::channel();
    let host = Host {
        out: Vec::new(),
        engine: None,
        pending: VecDeque::new(),
        unloaded_memory: || Some(1),
    };
    (host, tx, rx)
}

fn frames(bytes: &[u8]) -> Vec<Frame> {
    let mut cursor = bytes;
    let mut out = Vec::new();
    while let Ok((f, _)) = read_frame(&mut cursor) {
        out.push(f);
    }
    out
}

#[test]
fn failed_replacement_leaves_the_host_unloaded() {
    let (mut host, _tx, rx) = rig();
    assert!(host.handle(Frame::request(1, "load", json!({"model": "m"})), true, &rx));
    assert!(host.handle(
        Frame::request(2, "load", json!({"model": "missing"})),
        true,
        &rx
    ));
    assert!(host.engine.is_none());
    for (id, model) in [(3, "limited-a"), (4, "limited-b")] {
        assert!(host.handle(
            Frame::request(id, "load", json!({"model": model})),
            true,
            &rx
        ));
        assert_eq!(host.engine.as_ref().map(|e| e.model.as_str()), Some(model));
    }
    assert!(host.handle(Frame::request(5, "unload", json!({})), true, &rx));
    assert_eq!(host.status(false).payload["loaded"], false);
}

#[test]
fn a_cancel_that_arrives_mid_generation_stops_it_and_is_acknowledged_after() {
    let (mut host, tx, rx) = rig();
    assert!(host.handle(Frame::request(1, "load", json!({"model": "m"})), true, &rx));
    // The cancel and a status are queued before the generation reads
    // the channel between its first two tokens.
    tx.send(Some(Ok(Frame::request(3, "status", json!({})))))
        .unwrap();
    tx.send(Some(Ok(Frame::request(
        4,
        "cancel",
        json!({"request_id": 2}),
    ))))
    .unwrap();
    tx.send(Some(Ok(Frame::request(5, "unload", json!({})))))
        .unwrap();
    assert!(host.handle(
        Frame::request(2, "generate", json!({"user": "hello"})),
        true,
        &rx
    ));
    let out = frames(&host.out);
    let names: Vec<(u64, &str)> = out.iter().map(|f| (f.id, f.name.as_str())).collect();
    assert_eq!(
        names,
        [(0, "loaded"), (1, "load"), (3, "status"), (2, "generate")],
        "{names:?}"
    );
    assert_eq!(out[2].payload["busy"], true);
    assert_eq!(out[2].payload["memory_bytes"], 7);
    assert_eq!(out[3].payload["finish_reason"], "cancelled");
    assert_eq!(out[3].payload["text"], "");
    // The cancel and the unload wait their turn, in order.
    let pending: Vec<&str> = host.pending.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(pending, ["cancel", "unload"]);
    while let Some(f) = host.pending.pop_front() {
        assert!(host.handle(f, true, &rx));
    }
    let out = frames(&host.out);
    assert_eq!(out[4].name, "cancel");
    assert_eq!(out[5].name, "unload");
    assert!(host.handle(Frame::request(6, "status", json!({})), true, &rx));
    let out = frames(&host.out);
    assert_eq!(out[6].payload["loaded"], false);
    assert_eq!(out[6].payload["memory_bytes"], 1, "the unloaded reading");
}

#[test]
fn a_full_generation_streams_one_partial_per_token() {
    let (mut host, _tx, rx) = rig();
    assert!(host.handle(Frame::request(1, "load", json!({"model": "m"})), true, &rx));
    assert!(host.handle(
        Frame::request(2, "generate", json!({"user": "abc"})),
        true,
        &rx
    ));
    let out = frames(&host.out);
    let partials: String = out
        .iter()
        .filter(|f| f.name == "partial" && f.id == 2)
        .map(|f| f.payload["text"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(partials, "abc");
    let last = out.last().unwrap();
    assert_eq!(last.name, "generate");
    assert_eq!(last.payload["tokens"], 3);
    assert_eq!(last.payload["finish_reason"], "stop");
    assert!(host.handle(
        Frame::request(3, "load", json!({"model": "missing"})),
        true,
        &rx
    ));
    let out = frames(&host.out);
    assert_eq!(out.last().unwrap().payload["code"], "model_missing");
}
