//! R4 with the local tiny.en: a twelve-minute fixture (the jfk clip sixty
//! times with a second of silence between takes) imports as thirty speech windows
//! into one item with the merged transcript, segments on absolute time,
//! progress events per chunk and the decoded audio retained; a cancel
//! mid-job leaves a cancelled item with the finished chunks; a file over
//! `[history] max_import_seconds` is refused after the probe naming the
//! limit; a container that is not what the upload declares is refused
//! naming both.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use common::{Daemon, Tree};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Takes of the clip in the fixture.
const REPEATS: usize = 60;

fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

fn tree_with_tiny(model: &Path) -> Tree {
    let tree = Tree::new();
    let models = tree.root().join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&models).unwrap();
    std::os::unix::fs::symlink(model, models.join("ggml-tiny.en.bin")).unwrap();
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_dettivod"))
        .parent()
        .unwrap()
        .to_path_buf();
    if !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status =
            std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .args(["build", "-q", "-p", "dettivo-engine-whisper"])
                .status()
                .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    ));
    tree
}

/// The clip `REPEATS` times with a second of silence after each take.
fn twelve_minutes(jfk: &Path, out: &Path) -> u64 {
    let mut reader = hound::WavReader::open(jfk).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.sample_rate, spec.channels), (16_000, 1));
    let clip: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let mut writer = hound::WavWriter::create(out, spec).unwrap();
    for _ in 0..REPEATS {
        for s in &clip {
            writer.write_sample(*s).unwrap();
        }
        for _ in 0..16_000 {
            writer.write_sample(0i16).unwrap();
        }
    }
    writer.finalize().unwrap();
    (clip.len() as u64 + 16_000) * REPEATS as u64
}

fn upload(daemon: &Daemon, content_type: &str, bytes: &[u8]) -> String {
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": content_type, "size_hint": bytes.len()}),
    );
    let id = begun["transfer_id"].as_str().unwrap().to_string();
    let mut total = 0;
    for (i, chunk) in bytes.chunks(512 * 1024).enumerate() {
        daemon.result(
            "transfer.chunk",
            json!({"transfer_id": id, "seq": i + 1, "data_b64": base64::engine::general_purpose::STANDARD.encode(chunk)}),
        );
        total += 1;
    }
    daemon.result(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": total, "sha256": format!("{:x}", Sha256::digest(bytes))}),
    );
    id
}

fn import(daemon: &Daemon, transfer: &str, filename: &str) -> Value {
    daemon.request(
        "transcripts.import",
        json!({"transfer_id": transfer, "target_kind": "dictation", "filename": filename, "language": "en", "mode": "raw"}),
    )
}

fn subscribe(daemon: &Daemon) -> BufReader<UnixStream> {
    let stream = UnixStream::connect(daemon.tree.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(600)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let req = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": ["job.progress"], "buffer": 256}});
    (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    reader
}

fn next_event(reader: &mut BufReader<UnixStream>) -> Option<Value> {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return None;
    }
    let v: Value = serde_json::from_str(line.trim_end()).unwrap();
    Some(v["params"]["payload"].clone())
}

fn wait_settled(daemon: &Daemon, id: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        let listed = daemon.result(
            "transcripts.list",
            json!({"kinds": ["dictation"], "limit": 100, "cursor": null}),
        );
        let row = listed["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["ref"]["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} not listed: {listed}"));
        if row["status"] != "transcribing" {
            return row;
        }
        assert!(Instant::now() < deadline, "{id} still transcribing");
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[test]
fn a_twelve_minute_import_uses_speech_windows_and_a_cancel_keeps_finished_chunks() {
    let Some((model, jfk)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = tree_with_tiny(&model);
    let fixture = tree.root().join("twelve.wav");
    let samples = twelve_minutes(&jfk, &fixture);
    let bytes = std::fs::read(&fixture).unwrap();
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1")]);

    // A container that is not what the upload declares.
    let wrong = upload(&daemon, "audio/flac", &bytes);
    let refused = import(&daemon, &wrong, "twelve.flac");
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(
        refused["error"]["message"],
        "the upload declares audio/flac but its container is wave"
    );

    // Over the limit: refused after the probe, before any decoding.
    daemon.result(
        "config.set",
        json!({"key": "history.max_import_seconds", "value": "60"}),
    );
    let long = upload(&daemon, "audio/wav", &bytes);
    let refused = import(&daemon, &long, "twelve.wav");
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("60 seconds") && message.contains("history.max_import_seconds"),
        "{message}"
    );
    daemon.result("config.unset", json!({"key": "history.max_import_seconds"}));

    // The short default windows still publish exact progress and preserve every take.
    let mut events = subscribe(&daemon);
    let transfer = upload(&daemon, "audio/wav", &bytes);
    let started = import(&daemon, &transfer, "twelve.wav")["result"].clone();
    assert_eq!(started["job"]["state"], "running", "{started}");
    let job_id = started["job"]["job_id"].as_str().unwrap().to_string();
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    let mut seen: Vec<(String, Option<u64>, Option<u64>, f64)> = Vec::new();
    while let Some(p) = next_event(&mut events) {
        if p["job_id"] != job_id {
            continue;
        }
        let stage = p["stage"].as_str().unwrap_or("").to_string();
        seen.push((
            stage.clone(),
            p["chunks_done"].as_u64(),
            p["chunks_total"].as_u64(),
            p["progress"].as_f64().unwrap(),
        ));
        if matches!(stage.as_str(), "done" | "failed" | "cancelled") {
            break;
        }
    }
    assert_eq!(
        seen.first().map(|s| s.0.as_str()),
        Some("decoding"),
        "{seen:?}"
    );
    let chunk_events: Vec<&(String, Option<u64>, Option<u64>, f64)> =
        seen.iter().filter(|s| s.0 == "transcribing").collect();
    assert_eq!(
        chunk_events
            .iter()
            .map(|s| (s.1.unwrap(), s.2.unwrap()))
            .collect::<Vec<_>>(),
        (0..=30).map(|done| (done, 30)).collect::<Vec<_>>(),
        "{seen:?}"
    );
    assert!(seen.iter().any(|s| s.0 == "merging"), "{seen:?}");
    let last = seen.last().unwrap();
    assert_eq!((last.0.as_str(), last.3), ("done", 1.0), "{seen:?}");
    let row = wait_settled(&daemon, &id);
    assert_eq!(row["status"], "completed", "{row}");
    assert_eq!(row["duration_seconds"], samples.div_ceil(16_000));
    assert_eq!(row["title"], "twelve");
    let got = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    let text = got["text_polish"].as_str().unwrap().to_lowercase();
    let takes = text.matches("fellow americans").count();
    assert!(
        (REPEATS - 3..=REPEATS).contains(&takes),
        "{takes} takes recognised: {text}"
    );
    assert!(
        text.matches("ask not").count() <= REPEATS,
        "a take was duplicated across an overlap: {text}"
    );
    let segments = got["segments"].as_array().unwrap();
    assert!(segments.len() >= REPEATS, "{} segments", segments.len());
    let mut previous = 0;
    for (i, s) in segments.iter().enumerate() {
        assert_eq!(s["index"], i);
        assert_eq!(s["source_type"], "microphone");
        assert!(s["words"].is_null(), "whisper aligns no words: {s}");
        let start = s["start_ms"].as_u64().unwrap();
        assert!(start >= previous, "segment {i} out of order");
        assert!(s["end_ms"].as_u64().unwrap() <= 721_000);
        previous = start;
    }
    assert!(
        segments.last().unwrap()["start_ms"].as_u64().unwrap() > 700_000,
        "the last take sits at the end of the audio"
    );
    let retained = daemon
        .tree
        .root()
        .join("data/dettivo/dictations")
        .join(&id)
        .join("microphone.wav");
    let reader = hound::WavReader::open(&retained).expect("decoded audio retained");
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(u64::from(reader.len()), samples);
    let metadata: Value = serde_json::from_str(
        &std::fs::read_to_string(retained.with_file_name("metadata.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["source_kind"], "audioImport");
    assert!(metadata["segments"].as_array().unwrap().len() >= REPEATS);
    assert!(
        !daemon
            .tree
            .root()
            .join("cache/dettivo/transfers")
            .join(&transfer)
            .exists(),
        "the spooled upload is gone"
    );

    // Cancel after the first chunk: the item is cancelled and keeps it.
    let mut events = subscribe(&daemon);
    let transfer = upload(&daemon, "audio/wav", &bytes);
    let started = import(&daemon, &transfer, "again.wav")["result"].clone();
    let job_id = started["job"]["job_id"].as_str().unwrap().to_string();
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    while let Some(p) = next_event(&mut events) {
        if p["job_id"] == job_id && p["chunks_done"] == 1 {
            break;
        }
    }
    let cancelled = daemon.result(
        "transcripts.cancel",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    assert_eq!(cancelled["job"]["state"], "cancelled");
    assert_eq!(cancelled["job"]["job_id"], job_id);
    assert_eq!(cancelled["ref"]["id"], id);
    let row = wait_settled(&daemon, &id);
    assert_eq!(row["status"], "cancelled", "{row}");
    // The cancel lands while the next chunk may already be running; the
    // job stops after it, so one or two chunks are done, never every window.
    while let Some(p) = next_event(&mut events) {
        if p["job_id"] == job_id && p["stage"] == "cancelled" {
            let done = p["chunks_done"].as_u64().unwrap();
            assert!((1..=2).contains(&done), "{p}");
            assert!(p["progress"].as_f64().unwrap() < 1.0);
            break;
        }
    }
    let got = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    let text = got["text_polish"].as_str().unwrap().to_lowercase();
    let takes = text.matches("fellow americans").count();
    // One or two <=30 s windows cover two to five phrase starts; the
    // sixth starts at 60 s, beyond either window.
    assert!(
        (2..=5).contains(&takes),
        "the finished chunks' takes, never all sixty: {takes}"
    );
    let again = daemon.request(
        "transcripts.cancel",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    assert_eq!(again["error"]["data"]["app_code"], "NOT_FOUND");
    assert_eq!(
        again["error"]["message"],
        format!("no job is working on dictation {id}")
    );
    let stats = daemon.result("transcripts.stats", json!({}));
    assert_eq!(
        stats["audio_items"], 2,
        "both imports kept their decoded audio"
    );
    daemon.stop();
}
