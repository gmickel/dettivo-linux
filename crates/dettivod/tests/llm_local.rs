//! R3 and R5 against a live daemon with the Qwen3 1.7B catalogue file on
//! disk (`scripts/models/fetch-llm-test-model.sh`) and, for the dictation,
//! tiny.en with the jfk fixture: the `local` provider comes up under
//! `auto`, `polish.test` in `enhanced` mode returns a guarded rewrite
//! from the real engine, a dictation in `enhanced` mode inserts it, and
//! the Polish goldens run through the engine into a report that records
//! how the rewrites align with the macOS cases (never gated: sampling
//! differs), and the seeded roadmap meeting is analysed through the real
//! engine into a summary, decisions and action items (ADR 0036). Skipped
//! by name when the model is absent.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// A real engine on the CPU backend loads and answers inside this window
/// even on a busy machine; the daemon's own polish budget stays the product
/// bound.
const ENGINE_WINDOW: Duration = Duration::from_secs(180);

use common::{DAEMON, Daemon, Tree};
use serde_json::{Value, json};

/// One real engine at a time: the tests of this binary each spawn a
/// daemon with its own `dettivo-engine-llm`, and two of them on one
/// machine starve each other past the read window.
static ENGINE: Mutex<()> = Mutex::new(());

fn one_engine() -> MutexGuard<'static, ()> {
    ENGINE.lock().unwrap_or_else(|p| p.into_inner())
}

const LLM_FILE: &str = "Qwen3-1.7B-Q4_K_M.gguf";
const LLM_SHA: &str = "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897";

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
}

fn local_llm() -> Option<PathBuf> {
    let p = data_dir()?
        .join("dettivo/models/llm/qwen3-1.7b")
        .join(LLM_FILE);
    p.is_file().then_some(p)
}

fn local_tiny() -> Option<(PathBuf, PathBuf)> {
    let data = data_dir()?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

fn build_engine() -> PathBuf {
    let bin_dir = Path::new(DAEMON).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-llm").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-llm"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    bin_dir
}

/// Links the local model in as a verified download under `[llm] model =
/// "qwen3-1.7b"`, with Ollama pointed at a closed port so `auto` has only
/// the local engine to find.
fn tree_with_llm(model: &Path, extra: &str) -> Tree {
    let tree = Tree::new();
    let dir = tree.root().join("data/dettivo/models/llm/qwen3-1.7b");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(model, dir.join(LLM_FILE)).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        format!(
            r#"{{"provider":"llm","id":"qwen3-1.7b","sha256":"{LLM_SHA}","size_bytes":1107409472,"downloaded_at":0,"catalogue_version":1,"verified":true}}"#
        ),
    )
    .unwrap();
    let bin_dir = build_engine();
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[engines.llm]\nbackend = \"cpu\"\nmax_tokens = 96\n[speech]\nmodel = \"tiny.en\"\n[llm]\nmodel = \"qwen3-1.7b\"\nollama_url = \"http://127.0.0.1:1\"\ntimeout_ms = 60000\n[dictation]\nlanguage = \"en\"\n{extra}",
        bin_dir.display()
    ));
    tree
}

const SAMPLE: &str = "um so basically i think we should probably ship this thing tomorrow you know";

#[test]
fn the_local_provider_rewrites_a_polish_test_through_the_real_engine() {
    let Some(model) = local_llm() else {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
        return;
    };
    let _engine = one_engine();
    let daemon = Daemon::spawn(tree_with_llm(&model, ""), &[("DETTIVO_QA_MODE", "1")]);
    let providers = daemon.result("llm.providers.list", json!({}));
    assert_eq!(providers["selected"], "local", "{providers}");
    assert_eq!(providers["providers"][0]["available"], true, "{providers}");
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], true);

    let result = daemon.result_within(
        "polish.test",
        json!({"input": SAMPLE, "preset": "generic", "rules": [], "mode": "enhanced"}),
        ENGINE_WINDOW,
    );
    // The guards decide what goes in (FR-M7): the model's rewrite when it
    // earns its place, the deterministic text when the model changed
    // nothing a transform would have changed. What must never happen
    // here is a provider that did not answer.
    assert_ne!(
        result["notice"]["kind"], "provider_unavailable",
        "the real engine answers inside the budget: {result}"
    );
    assert!(
        result["model"] == "qwen3-1.7b" || result["model"] == "deterministic",
        "{result}"
    );
    let output = result["output"].as_str().unwrap();
    assert!(output.to_lowercase().contains("tomorrow"), "{result}");
    assert!(
        !output.contains("<think>") && !output.contains("um "),
        "{result}"
    );
    if result["model"] == "qwen3-1.7b" {
        assert_eq!(result["enhanced"], result["output"], "{result}");
    }

    // The engine is warm now and reports what it loaded.
    let engine = daemon.result("llm.engine.status", json!({}));
    assert_eq!(engine["running"], true, "{engine}");
    assert_eq!(engine["loaded"], true, "{engine}");
    assert_eq!(engine["backend"], "cpu", "{engine}");
    assert!(
        engine["model"].as_str().unwrap().ends_with(LLM_FILE),
        "{engine}"
    );
    // The log never carries the sample or the answer.
    let log = daemon.stop();
    assert!(!log.contains("ship this thing"), "{log}");
    assert!(!log.contains(output), "{log}");
}

#[test]
fn an_enhanced_dictation_inserts_the_local_rewrite() {
    let (Some(model), Some((tiny, wav))) = (local_llm(), local_tiny()) else {
        eprintln!("skip: llm/qwen3-1.7b, tiny.en or jfk.wav missing");
        return;
    };
    let _engine = one_engine();
    let tree = tree_with_llm(&model, "");
    let whisper = tree.root().join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&whisper).unwrap();
    std::os::unix::fs::symlink(&tiny, whisper.join("ggml-tiny.en.bin")).unwrap();
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let started = daemon.result(
        "dictation.start",
        json!({"language": "en", "mode": "enhanced"}),
    );
    assert_eq!(started["job"]["state"], "running", "{started}");
    std::thread::sleep(Duration::from_millis(4000));
    let stopped = daemon.result("dictation.stop", json!({}));
    assert_eq!(stopped["job"]["state"], "succeeded", "{stopped}");
    let item = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": stopped["ref"]["id"]}}),
    );
    assert_eq!(item["mode"], "enhanced", "{item}");
    assert_ne!(
        item["notice"]["kind"], "provider_unavailable",
        "the local engine answered: {item}"
    );
    let inserted = item["text_polish"].as_str().unwrap();
    assert!(
        inserted.contains("Americans"),
        "the guard keeps the words: {item}"
    );
    assert!(!inserted.contains("<think>"), "{item}");
    let log = daemon.stop();
    assert!(log.contains("dictation text step"), "{log}");
    assert!(
        log.contains("dettivo-engine-llm"),
        "the local engine was spawned: {log}"
    );
}

/// The Polish goldens through the real engine, recorded as
/// `target/llm-goldens-report.json` (checked in as
/// `docs/reports/llm-goldens-report.json`): per case the input, the
/// pipeline's own deterministic text, the macOS expectation (which the
/// Swift suite reaches with the case's own transform set), the engine's
/// answer, the model that answered and whether the answer agrees with
/// each once punctuation and case are set aside. Alignment is reported,
/// never asserted.
#[test]
fn the_polish_goldens_run_through_the_engine_into_a_report() {
    let Some(model) = local_llm() else {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
        return;
    };
    let _engine = one_engine();
    let goldens = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-language/tests/goldens/polish_fallback.json");
    let suite: Value = serde_json::from_str(&std::fs::read_to_string(goldens).unwrap()).unwrap();
    let daemon = Daemon::spawn(tree_with_llm(&model, ""), &[("DETTIVO_QA_MODE", "1")]);
    let normalise = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<Vec<char>>()
            .into_iter()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut rows = Vec::new();
    let mut agree = 0;
    let mut agree_deterministic = 0;
    let mut rewritten = 0;
    for case in suite["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap_or_default();
        let deterministic = daemon.result_within(
            "polish.test",
            json!({"input": input, "preset": "generic", "rules": [], "mode": "deterministic_polish"}),
            ENGINE_WINDOW,
        );
        let deterministic = deterministic["output"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let result = daemon.result_within(
            "polish.test",
            json!({"input": input, "preset": "generic", "rules": [], "mode": "enhanced"}),
            ENGINE_WINDOW,
        );
        let output = result["output"].as_str().unwrap_or_default().to_string();
        let aligned = normalise(&output) == normalise(expected);
        let aligned_deterministic = normalise(&output) == normalise(&deterministic);
        agree += usize::from(aligned);
        agree_deterministic += usize::from(aligned_deterministic);
        rewritten += usize::from(result["model"] == "qwen3-1.7b");
        assert_ne!(
            result["notice"]["kind"], "provider_unavailable",
            "the engine must answer: {result}"
        );
        rows.push(json!({
            "name": case["name"],
            "input": input,
            "deterministic": deterministic,
            "expected_macos": expected,
            "output": output,
            "model": result["model"],
            "notice": result["notice"],
            "aligned_with_deterministic": aligned_deterministic,
            "aligned_with_macos": aligned,
        }));
    }
    let report = json!({
        "model": "llm/qwen3-1.7b",
        "engine": "dettivo-engine-llm (cpu)",
        "preset": "generic",
        "cases": rows.len(),
        "rewritten_by_the_model": rewritten,
        "aligned_with_deterministic": agree_deterministic,
        "aligned_with_macos": agree,
        "note": "alignment is recorded, not gated: sampling differs from the macOS goldens, the Swift suite runs each case with its own transform set while polish.test runs the generic preset, and the guards decide what is inserted",
        "rows": rows,
    });
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/llm-goldens-report.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
    eprintln!(
        "llm goldens: {} of {} aligned, {} rewritten by the model; report at {}",
        agree,
        rows.len(),
        rewritten,
        out.display()
    );
    daemon.stop();
}

/// R2 of the notes spec: `meetings.analyze` over the seeded roadmap
/// meeting through the real engine lands a parsed analysis on the row.
/// The words are the model's; what is asserted is that the strict parser
/// accepted an answer from the real engine inside the budget and the
/// row carries it.
#[test]
fn the_seeded_meeting_is_analysed_through_the_real_engine() {
    let Some(model) = local_llm() else {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
        return;
    };
    let _engine = one_engine();
    let tree = tree_with_llm(&model, "[meetings.analysis]\ntimeout_ms = 180000\n");
    // A JSON analysis needs more room than a one-line rewrite.
    let config = std::fs::read_to_string(tree.config_file()).unwrap();
    std::fs::write(
        tree.config_file(),
        config.replace("max_tokens = 96", "max_tokens = 768"),
    )
    .unwrap();
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")]);
    const RICH: &str = "5eed0000-0000-4000-8000-00000000a001";
    let started = daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    assert_eq!(started["job"]["state"], "running", "{started}");
    let deadline = std::time::Instant::now() + Duration::from_secs(240);
    let done = loop {
        let got = daemon.result("meetings.analysis.get", json!({"meeting_id": RICH}));
        if got["analysis_status"] != "running" {
            break got;
        }
        assert!(std::time::Instant::now() < deadline, "{got}");
        std::thread::sleep(Duration::from_millis(250));
    };
    assert_eq!(done["analysis_status"], "ready", "{done}");
    assert_eq!(done["model"], "qwen3-1.7b");
    let summary = done["analysis"]["summary"].as_str().unwrap();
    assert!(summary.len() > 20 && !summary.contains("<think>"), "{done}");
    assert!(done["analysis"]["decisions"].is_array());
    assert!(done["analysis"]["action_items"].is_array());
    let log = daemon.stop();
    assert!(log.contains("meeting analysis ready"), "{log}");
    assert!(!log.contains(summary), "the log never carries the analysis");
}
