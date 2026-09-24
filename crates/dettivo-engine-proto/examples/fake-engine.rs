//! A scripted engine for supervisor tests: loads instantly, answers
//! `recognize` with the request's language as text, answers `generate`
//! with the user text upper-cased after one `partial` (a user text of
//! `__SLOW__` sleeps two seconds first, `__HANG__` thirty seconds past any
//! caller's patience, and stalls twenty seconds on a `recognize` whose
//! language is `stall`; it crashes on demand (a
//! `crash-on-load` file next to the binary, or a `recognize` whose language
//! is `crash`, or a `generate` whose user text is `__CRASH__`, or a
//! `diarize` while a `crash-on-diarize` file sits next to the binary).
//! `diarize` answers one turn per second of audio, alternating two
//! speakers, after one `progress` event. Speaks the protocol on
//! stdin/stdout exactly like a real engine.

use std::io::{self, BufReader, BufWriter, Write};

use dettivo_engine_proto::{Frame, FrameError, read_frame, write_frame};
use serde_json::json;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut out = BufWriter::new(stdout.lock());
    let mut model: Option<String> = None;
    eprintln!("fake-engine: ready pid={}", std::process::id());
    loop {
        let (frame, attachments) = match read_frame(&mut reader) {
            Ok(x) => x,
            Err(FrameError::Eof) => break,
            Err(e) => {
                eprintln!("fake-engine: {e}");
                break;
            }
        };
        let id = frame.id;
        let reply = match frame.name.as_str() {
            "load" => {
                let dir = std::env::current_exe()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf();
                let preference = frame.payload["backend_preference"]
                    .as_str()
                    .unwrap_or("auto");
                if dir.join("record-loads").exists() {
                    let mut log = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(dir.join("loads.log"))
                        .unwrap();
                    writeln!(log, "{preference}").unwrap();
                }
                if preference != "cpu" && std::env::var("DETTIVO_FORCE_CPU").as_deref() != Ok("1") {
                    if dir.join("malformed-on-gpu-load").exists() {
                        out.write_all(b"[New Thread]").unwrap();
                        out.flush().unwrap();
                        std::thread::sleep(std::time::Duration::from_secs(20));
                        std::process::exit(3);
                    }
                    if dir.join("crash-on-gpu-load").exists() {
                        std::process::exit(3);
                    }
                }
                let marker = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("crash-on-load")));
                if marker.is_some_and(|m| m.exists()) {
                    eprintln!("fake-engine: crashing on load");
                    std::process::exit(3);
                }
                let m = frame.payload["model"].as_str().unwrap_or("").to_string();
                model = Some(m.clone());
                let loaded = json!({"model": m, "backend": "cpu", "reason": "fake engine"});
                let _ = write_frame(&mut out, &Frame::event(0, "loaded", loaded.clone()), &[]);
                Frame::response(id, "load", loaded)
            }
            "unload" => {
                model = None;
                Frame::response(id, "unload", json!({}))
            }
            "status" => Frame::response(
                id,
                "status",
                json!({"loaded": model.is_some(), "model": model, "backend": model.as_ref().map(|_| "cpu"), "busy": false}),
            ),
            "cancel" => Frame::response(id, "cancel", json!({})),
            "generate" => {
                let user = frame.payload["user"].as_str().unwrap_or("").to_string();
                if user == "__CRASH__" {
                    eprintln!("fake-engine: crashing on generate");
                    std::process::exit(5);
                }
                if user == "__SLOW__" {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                if user == "__HANG__" {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                }
                let text = user.to_uppercase();
                let _ = write_frame(
                    &mut out,
                    &Frame::event(id, "partial", json!({"request_id": id, "text": text})),
                    &[],
                );
                Frame::response(
                    id,
                    "generate",
                    json!({
                        "text": text,
                        "tokens": user.split_whitespace().count(),
                        "finish_reason": "stop",
                        "backend": "cpu"
                    }),
                )
            }
            "diarize" => {
                let marker = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("crash-on-diarize")));
                if marker.is_some_and(|m| m.exists()) {
                    eprintln!("fake-engine: crashing on diarize");
                    std::process::exit(6);
                }
                let seconds = attachments.first().map(|a| a.len() / 32_000).unwrap_or(0) as u64;
                let _ = write_frame(
                    &mut out,
                    &Frame::event(
                        id,
                        "progress",
                        json!({"request_id": id, "fraction": 0.5, "completed": 1, "total": 2}),
                    ),
                    &[],
                );
                let turns: Vec<_> = (0..seconds)
                    .map(|i| {
                        json!({"start_ms": i * 1000, "end_ms": (i + 1) * 1000, "speaker": format!("SPEAKER_{:02}", i % 2)})
                    })
                    .collect();
                Frame::response(id, "diarize", json!({"turns": turns}))
            }
            "recognize" => {
                let language = frame.payload["language"]
                    .as_str()
                    .unwrap_or("auto")
                    .to_string();
                if language == "crash" {
                    eprintln!("fake-engine: crashing on request");
                    std::process::exit(4);
                }
                if language == "stall" {
                    eprintln!("fake-engine: stalling on request");
                    std::thread::sleep(std::time::Duration::from_secs(20));
                }
                let samples = attachments.first().map(|a| a.len() / 2).unwrap_or(0) as u64;
                Frame::response(
                    id,
                    "recognize",
                    json!({
                        "text": format!("fake {language} pid={}", std::process::id()),
                        "language": language,
                        "segments": [{"start_ms": 0, "end_ms": samples * 1000 / 16_000, "text": "fake"}],
                        "duration_ms": samples * 1000 / 16_000,
                        "backend": "cpu"
                    }),
                )
            }
            other => Frame::response(
                id,
                "error",
                json!({"request_id": id, "code": "bad_request", "message": format!("unknown request {other:?}")}),
            ),
        };
        if write_frame(&mut out, &reply, &[]).is_err() {
            break;
        }
    }
}
