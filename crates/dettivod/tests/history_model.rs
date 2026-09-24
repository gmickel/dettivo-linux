//! R4 and R5 with the mock microphone and the local tiny.en: a dictation
//! ends with an item in the store carrying the transcript, insertion
//! result, app id, engine and model and its retained audio; after a
//! restart `dictation.reinsert_last` inserts it again through the mock
//! path; a re-run through the real engine produces a linked item and
//! reports progress; an audio import transcribes and a second one is a
//! conflict; a busy database is reported on the final state while the
//! insertion stands.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use common::{Daemon, Tree};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

fn tree_with_tiny(model: &Path, extra: &str) -> Tree {
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
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n{extra}",
        bin_dir.display()
    ));
    tree
}

fn qa_env(wav: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("DETTIVO_QA_MODE", "1".into()),
        ("DETTIVO_MOCK_MIC", wav.to_string_lossy().into_owned()),
        ("DETTIVO_MOCK_INSERT", "1".into()),
    ]
}

fn spawn(tree: Tree, env: &[(&'static str, String)]) -> Daemon {
    let borrowed: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    Daemon::spawn(tree, &borrowed)
}

/// Dictates most of the fixture and returns the stop result.
fn dictate(daemon: &Daemon) -> Value {
    let started = daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(started["job"]["state"], "running");
    std::thread::sleep(Duration::from_millis(6500));
    daemon.result("dictation.stop", json!({}))
}

/// Polls the listing until `id` leaves `transcribing`; returns its row.
fn wait_settled(daemon: &Daemon, id: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(120);
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
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn subscribe(daemon: &Daemon, topics: &[&str]) -> BufReader<UnixStream> {
    let stream = UnixStream::connect(daemon.tree.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let req = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": topics, "buffer": 256}});
    (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    reader
}

fn events_until(reader: &mut BufReader<UnixStream>, stop: impl Fn(&Value) -> bool) -> Vec<Value> {
    let mut out = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return out;
        }
        let v: Value = serde_json::from_str(line.trim_end()).unwrap();
        let params = v["params"].clone();
        let done = stop(&params);
        out.push(params);
        if done {
            return out;
        }
    }
}

#[test]
fn a_dictation_is_stored_with_audio_reinserts_after_a_restart_and_reruns_through_the_engine() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = tree_with_tiny(&model, "");
    let env = qa_env(&wav);
    let daemon = spawn(tree, &env);
    let stopped = dictate(&daemon);
    let id = stopped["ref"]["id"].as_str().unwrap().to_string();

    // The item is in the store with every fact the spec names.
    let latest = daemon.result("transcripts.latest", json!({"kind": "any"}));
    assert_eq!(latest["ref"]["id"], id);
    let got = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    let text = got["text_polish"].as_str().unwrap().to_lowercase();
    assert!(
        text.contains("fellow americans") && text.contains("country"),
        "{text}"
    );
    assert!(
        got["text_raw"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    let row = wait_settled(&daemon, &id);
    assert_eq!(row["status"], "completed");
    assert!(row["duration_seconds"].as_u64().unwrap() >= 5);
    let stats = daemon.result("transcripts.stats", json!({}));
    assert_eq!(stats["item_count"], 1);
    assert_eq!(stats["audio_items"], 1);
    let item_dir = daemon.tree.root().join("data/dettivo/dictations").join(&id);
    assert!(item_dir.join("microphone.wav").is_file(), "audio retained");
    let metadata: Value =
        serde_json::from_str(&std::fs::read_to_string(item_dir.join("metadata.json")).unwrap())
            .unwrap();
    assert_eq!(metadata["stt_provider"], "whisper");
    assert_eq!(metadata["stt_model"], "tiny.en");
    assert_eq!(
        metadata["app_id"], "org.gnome.TextEditor",
        "the QA focus probe's fixed target"
    );
    assert_eq!(metadata["insertion"]["outcome"], "inserted");
    assert_eq!(metadata["source_kind"], "dictation");
    assert!(
        !daemon
            .tree
            .root()
            .join("state/dettivo/sessions/job_dict_1/microphone.wav")
            .exists(),
        "the take moved out of the session directory"
    );
    let inserted = daemon.tree.root().join("state/dettivo/qa/inserted.txt");
    assert_eq!(
        std::fs::read_to_string(&inserted).unwrap().lines().count(),
        1
    );
    let (log, tree) = daemon.stop_keep();
    assert!(log.contains("dictation stored"), "{log}");

    // After a restart the session has no last transcript; the store has.
    let daemon = spawn(tree, &env);
    let reinserted = daemon.result("dictation.reinsert_last", json!({}));
    assert_eq!(reinserted["ref"]["id"], id);
    assert_eq!(reinserted["insertion"]["outcome"], "inserted");
    let lines: Vec<String> = std::fs::read_to_string(&inserted)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert!(lines[0].to_lowercase().contains("americans"));

    // A re-run through the real engine: a linked item, progress events.
    let mut progress = subscribe(&daemon, &["job.progress"]);
    let rerun = daemon.result(
        "transcripts.rerun",
        json!({"ref": {"kind": "dictation", "id": id}, "model": "tiny.en"}),
    );
    assert_eq!(rerun["rerun_of"]["id"], id);
    assert_eq!(rerun["job"]["state"], "running");
    assert_eq!(rerun["job"]["job_id"], "job_rerun_1");
    let new_id = rerun["ref"]["id"].as_str().unwrap().to_string();
    assert_ne!(new_id, id);
    let health = daemon.result("system.health", json!({}));
    assert!(
        health["active_jobs"].as_u64().unwrap() >= 1
            || wait_settled(&daemon, &new_id)["status"] == "completed"
    );
    let row = wait_settled(&daemon, &new_id);
    assert_eq!(row["status"], "completed", "{row}");
    let events = events_until(&mut progress, |p| {
        p["payload"]["job_id"] == "job_rerun_1" && p["payload"]["progress"] == 1.0
    });
    assert!(
        events.iter().any(|p| p["payload"]["progress"] == 0.0),
        "{events:?}"
    );
    let got = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": new_id}}),
    );
    assert!(
        got["text_polish"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    let segments = got["segments"].as_array().unwrap();
    assert!(
        !segments.is_empty(),
        "a re-run goes through the chunked job: {got}"
    );
    assert_eq!(segments[0]["source_type"], "microphone");
    let out = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/json", "size_hint": 0}),
    )["transfer_id"]
        .as_str()
        .unwrap()
        .to_string();
    daemon.result(
        "transcripts.export",
        json!({"format": "json", "transfer_id": out, "ref": {"kind": "dictation", "id": new_id}}),
    );
    let chunk = daemon.result("transfer.pull", json!({"transfer_id": out, "seq": 1}));
    let doc: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(chunk["data_b64"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(doc["items"][0]["source_kind"], "rerun");
    assert_eq!(doc["items"][0]["rerun_of_item_id"], id);
    assert_eq!(doc["items"][0]["stt_model"], "tiny.en");
    assert!(
        doc["items"][0]["audio_path"].is_string(),
        "the re-run keeps a copy of the audio"
    );
    let missing = daemon.request(
        "transcripts.rerun",
        json!({"ref": {"kind": "dictation", "id": id}, "model": "base"}),
    );
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dettivo speech download --model base")
    );

    // Without retained audio a re-run is NOT_FOUND with the reason.
    daemon.result(
        "config.set",
        json!({"key": "history.keep_audio", "value": "false"}),
    );
    let stopped = dictate(&daemon);
    let bare = stopped["ref"]["id"].as_str().unwrap().to_string();
    let refused = daemon.request(
        "transcripts.rerun",
        json!({"ref": {"kind": "dictation", "id": bare}, "model": "tiny.en"}),
    );
    assert_eq!(refused["error"]["code"], -32012);
    assert_eq!(refused["error"]["data"]["app_code"], "NOT_FOUND");
    assert_eq!(
        refused["error"]["data"]["details"]["reason"],
        "audio_not_retained"
    );
    assert_eq!(
        refused["error"]["message"],
        format!("dictation {bare} has no retained audio")
    );
    assert!(
        !daemon
            .tree
            .root()
            .join("data/dettivo/dictations")
            .join(&bare)
            .join("microphone.wav")
            .exists()
    );
    daemon.stop();
}

#[test]
fn an_audio_import_transcribes_and_a_second_one_is_a_conflict() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let daemon = spawn(tree_with_tiny(&model, ""), &qa_env(&wav));
    let bytes = std::fs::read(&wav).unwrap();
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "audio/wav", "size_hint": bytes.len()}),
    );
    let transfer = begun["transfer_id"].as_str().unwrap().to_string();
    let mut total = 0;
    for (i, chunk) in bytes.chunks(128 * 1024).enumerate() {
        daemon.result(
            "transfer.chunk",
            json!({"transfer_id": transfer, "seq": i + 1, "data_b64": base64::engine::general_purpose::STANDARD.encode(chunk)}),
        );
        total += 1;
    }
    let early = daemon.request(
        "transcripts.import",
        json!({"transfer_id": transfer, "target_kind": "dictation", "filename": "jfk.wav", "language": "en", "mode": "raw"}),
    );
    assert_eq!(
        early["error"]["data"]["app_code"], "CONFLICT",
        "not committed yet: {early}"
    );
    daemon.result(
        "transfer.commit",
        json!({"transfer_id": transfer, "total_chunks": total, "sha256": format!("{:x}", Sha256::digest(&bytes))}),
    );
    let imported = daemon.result(
        "transcripts.import",
        json!({"transfer_id": transfer, "target_kind": "dictation", "filename": "../jfk.wav", "language": "en", "mode": "raw"}),
    );
    assert_eq!(imported["job"]["state"], "running");
    assert_eq!(imported["job"]["job_id"], "job_import_1");
    assert_eq!(imported["is_partial"], true);
    let id = imported["ref"]["id"].as_str().unwrap().to_string();
    // The fixture: a second import while one runs.
    let conflict = daemon.request(
        "transcripts.import",
        json!({"transfer_id": "xfer_in_1", "target_kind": "dictation", "filename": "note.wav", "language": "en", "mode": "raw"}),
    );
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../dettivo-proto/fixtures/transcripts/import.error-conflict-already-in-progress.json",
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(conflict["error"], expected["error"]["error"]);
    let row = wait_settled(&daemon, &id);
    assert_eq!(row["status"], "completed", "{row}");
    assert_eq!(row["title"], "jfk");
    let got = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": id}}),
    );
    assert!(
        got["text_polish"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    assert!(
        daemon
            .tree
            .root()
            .join("data/dettivo/dictations")
            .join(&id)
            .join("microphone.wav")
            .is_file()
    );
    daemon.stop();
}

#[test]
fn a_busy_database_is_reported_on_the_final_state_while_the_insertion_stands() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let daemon = spawn(tree_with_tiny(&model, ""), &qa_env(&wav));
    let mut states = subscribe(&daemon, &["dictation.state"]);
    daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    std::thread::sleep(Duration::from_millis(3000));
    let db = daemon.tree.root().join("data/dettivo/dettivo.db");
    let holder = rusqlite::Connection::open(&db).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    // The stop waits out the five second busy timeout, so a patient client.
    let stream = UnixStream::connect(daemon.tree.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    (&stream)
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":\"stop\",\"method\":\"dictation.stop\",\"params\":{}}\n",
        )
        .unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(line.trim_end()).unwrap();
    let stopped = reply["result"].clone();
    assert_eq!(stopped["job"]["state"], "succeeded", "{stopped}");
    holder.execute_batch("COMMIT").unwrap();
    let events = events_until(&mut states, |p| p["payload"]["state"] == "idle");
    let idle = events
        .iter()
        .find(|p| p["payload"]["state"] == "idle")
        .unwrap();
    let reason = idle["payload"]["reason"].as_str().unwrap_or("");
    assert!(
        reason.starts_with("history:") && reason.contains("busy"),
        "{idle}"
    );
    let inserted = daemon.tree.root().join("state/dettivo/qa/inserted.txt");
    assert!(
        std::fs::read_to_string(&inserted)
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    let none = daemon.request("transcripts.latest", json!({"kind": "dictation"}));
    assert_eq!(
        none["error"]["data"]["app_code"], "NOT_FOUND",
        "the item was not stored"
    );
    let again = daemon.result("dictation.reinsert_last", json!({}));
    assert_eq!(
        again["ref"]["id"], stopped["ref"]["id"],
        "the session still holds it"
    );
    daemon.stop();
}
