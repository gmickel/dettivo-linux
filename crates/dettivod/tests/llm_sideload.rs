//! fn-30 R1 against a live daemon: `[llm] polish_experiment` binds the
//! sideloaded fine-tune the manifest names, `llm.models.status` lists it
//! with `source = "sideload"` and the selection mark, the catalogue
//! methods never know its id, an adapter whose base is missing reports
//! the base and its download command, a manifest that does not resolve
//! is reported by name with Enhanced left on `[llm] model`, and, with
//! the Qwen3 1.7B file on this machine standing in for the fine-tune, a
//! `polish.test` in `enhanced` mode names the sideload as its model.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{DAEMON, Daemon, Tree};
use serde_json::{Value, json};

const LLM_FILE: &str = "Qwen3-1.7B-Q4_K_M.gguf";

fn local_llm() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let p = data.join("dettivo/models/llm/qwen3-1.7b").join(LLM_FILE);
    p.is_file().then_some(p)
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

/// A tree whose experiments directory holds `current.json` in the macOS
/// shape over `tuned/<file>`, with `polish_experiment = "current"`.
fn tree_with_experiment(manifest: &str, gguf: Option<&Path>) -> Tree {
    let tree = Tree::new();
    let exp = tree.root().join("exp");
    std::fs::create_dir_all(exp.join("tuned")).unwrap();
    match gguf {
        Some(real) => std::os::unix::fs::symlink(real, exp.join("tuned/tuned.gguf")).unwrap(),
        None => std::fs::write(exp.join("tuned/tuned.gguf"), b"GGUF").unwrap(),
    }
    std::fs::write(exp.join("current.json"), manifest).unwrap();
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[engines.llm]\nbackend = \"cpu\"\nmax_tokens = 96\n[llm]\nprovider = \"local\"\nollama_url = \"http://127.0.0.1:1\"\ntimeout_ms = 60000\npolish_experiment = \"current\"\nexperiments_dir = \"{}\"\n",
        build_engine().display(),
        exp.display()
    ));
    tree
}

const FUSED: &str = r#"{"id":"qwen3-1.7b-private-best","displayName":"Current 1.7B Experiment","relativeModelPath":"tuned"}"#;
const LORA: &str = r#"{"id":"tuned-lora","displayName":"Tuned adapter","relativeModelPath":"tuned","format":"gguf-lora","baseModel":"qwen3-1.7b"}"#;

fn rows(daemon: &Daemon) -> Vec<Value> {
    daemon.result("llm.models.status", json!({}))["models"]
        .as_array()
        .cloned()
        .unwrap()
}

#[test]
fn a_fused_sideload_is_listed_selected_and_never_downloadable() {
    let daemon = Daemon::spawn(
        tree_with_experiment(FUSED, None),
        &[("DETTIVO_QA_MODE", "1")],
    );
    let status = daemon.result("llm.models.status", json!({}));
    assert_eq!(status["polish_experiment"], "current");
    assert_eq!(status["selected"], "qwen3-4b-instruct-2507");
    let models = status["models"].as_array().unwrap();
    let side = models
        .iter()
        .find(|m| m["source"] == "sideload")
        .expect("a sideload row");
    assert_eq!(side["id"], "qwen3-1.7b-private-best");
    assert_eq!(side["display_name"], "Current 1.7B Experiment");
    assert_eq!(side["format"], "gguf");
    assert_eq!(side["readiness"], "ready");
    assert_eq!(side["is_selected"], true);
    assert_eq!(side["available"], false);
    assert!(side["path"].as_str().unwrap().ends_with("tuned/tuned.gguf"));
    assert!(side.get("base_model").is_none(), "{side}");
    assert!(side.get("manifest_error").is_none(), "{side}");
    for m in models.iter().filter(|m| m["source"] == "catalogue") {
        assert_eq!(m["is_selected"], false, "{m}");
    }
    assert_eq!(models.len(), 5, "{status}");

    // Narrowed to the sideload id it answers the one row; the catalogue
    // methods answer NOT_FOUND naming the catalogue's ids.
    let one = daemon.result(
        "llm.models.status",
        json!({"model": "qwen3-1.7b-private-best"}),
    );
    assert_eq!(one["models"].as_array().unwrap().len(), 1);
    for method in [
        "llm.models.download",
        "llm.models.cancel",
        "llm.models.delete",
    ] {
        let err = daemon.request(method, json!({"model": "qwen3-1.7b-private-best"}));
        assert_eq!(
            err["error"]["data"]["app_code"], "NOT_FOUND",
            "{method}: {err}"
        );
        let message = err["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("qwen3-4b-instruct-2507"),
            "{method}: {err}"
        );
        assert!(
            message.contains("qwen3-1.7b-private-best"),
            "{method}: {err}"
        );
    }

    // The provider names the sideload.
    let providers = daemon.result("llm.providers.list", json!({}));
    let local = &providers["providers"][0];
    assert_eq!(local["id"], "local");
    assert_eq!(local["model"], "qwen3-1.7b-private-best", "{providers}");
    daemon.stop();
}

#[test]
fn an_adapter_over_a_missing_base_names_the_base_and_its_download() {
    let daemon = Daemon::spawn(
        tree_with_experiment(LORA, None),
        &[("DETTIVO_QA_MODE", "1")],
    );
    let side = rows(&daemon)
        .into_iter()
        .find(|m| m["source"] == "sideload")
        .unwrap();
    assert_eq!(side["id"], "tuned-lora");
    assert_eq!(side["format"], "gguf-lora");
    assert_eq!(side["base_model"], "qwen3-1.7b");
    assert_eq!(side["readiness"], "missing");
    let error = side["error"].as_str().unwrap();
    assert!(error.contains("llm/qwen3-1.7b"), "{side}");
    assert!(
        error.contains("dettivo llm download --model qwen3-1.7b"),
        "{side}"
    );
    let providers = daemon.result("llm.providers.list", json!({}));
    let local = &providers["providers"][0];
    assert_eq!(local["available"], false, "{providers}");
    assert!(
        local["detail"]
            .as_str()
            .unwrap()
            .contains("the base of experiment tuned-lora"),
        "{providers}"
    );
    assert_eq!(local["hint"], "dettivo llm download --model qwen3-1.7b");
    daemon.stop();
}

#[test]
fn an_unresolvable_manifest_is_reported_by_name_and_enhanced_stays_on_the_model() {
    let tree = tree_with_experiment(
        r#"{"id":"x","displayName":"X","relativeModelPath":"absent"}"#,
        None,
    );
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1")]);
    let status = daemon.result("llm.models.status", json!({}));
    let side = status["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["source"] == "sideload")
        .cloned()
        .unwrap();
    assert_eq!(side["id"], "current");
    assert_eq!(side["readiness"], "missing");
    assert!(
        side["manifest_error"]
            .as_str()
            .unwrap()
            .contains("absent is not a directory"),
        "{side}"
    );
    // Enhanced runs on [llm] model: the provider names the catalogue model.
    let providers = daemon.result("llm.providers.list", json!({}));
    assert_eq!(
        providers["providers"][0]["model"], "qwen3-4b-instruct-2507",
        "{providers}"
    );
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], false);
    daemon.stop();
}

#[test]
fn an_enhanced_polish_test_names_the_sideloaded_model() {
    let Some(model) = local_llm() else {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
        return;
    };
    let daemon = Daemon::spawn(
        tree_with_experiment(FUSED, Some(&model)),
        &[("DETTIVO_QA_MODE", "1")],
    );
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], true, "{caps}");
    let result = daemon.result(
        "polish.test",
        json!({
            "input": "um so basically i think we should probably ship this thing tomorrow you know",
            "preset": "generic",
            "rules": [],
            "mode": "enhanced"
        }),
    );
    assert_ne!(result["notice"]["kind"], "provider_unavailable", "{result}");
    assert!(
        result["model"] == "qwen3-1.7b-private-best" || result["model"] == "deterministic",
        "{result}"
    );
    let engine = daemon.result("llm.engine.status", json!({}));
    assert_eq!(engine["loaded"], true, "{engine}");
    assert!(
        engine["model"]
            .as_str()
            .unwrap()
            .ends_with("tuned/tuned.gguf"),
        "{engine}"
    );
    assert!(engine.get("lora").is_none(), "{engine}");
    daemon.stop();
}
