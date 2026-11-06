//! fn-95 end to end: `dettivo vocabulary import` against a daemon in a
//! temporary tree. The import adds a list's terms to the file and keeps
//! its comments and entries (R1); duplicates are skipped and reported, a
//! list that breaks a limit and a dry run write nothing (R2); the bench's
//! vocabulary imported from a file is the one written by hand (R4). The
//! take test (R3) runs on the local Parakeet v2 model and is skipped by
//! name when it is not on disk.

mod common;

use std::path::{Path, PathBuf};

use common::{Daemon, Tree, stderr, stdout};
use serde_json::{Value, json};

/// A daemon whose file is `config`, with nothing outside the tree.
fn daemon_with(config: &str) -> Daemon {
    let tree = Tree::new();
    tree.write_config(config);
    Daemon::spawn(tree, &[])
}

/// `list` written into the tree, for the CLI to import.
fn list_file(d: &Daemon, name: &str, list: &str) -> PathBuf {
    let path = d.tree.root().join(name);
    std::fs::write(&path, list).unwrap();
    path
}

fn file(d: &Daemon) -> String {
    std::fs::read_to_string(d.tree.config_file()).unwrap()
}

/// The vocabulary in force, as `config get` prints it with `--json`.
fn vocabulary(d: &Daemon) -> Value {
    let out = d.cli(&["--json", "config", "get", "dictation.vocabulary"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let got: Value = serde_json::from_str(&stdout(&out)).unwrap();
    got["entries"][0]["value"].clone()
}

const HAND_WRITTEN: &str = concat!(
    "# my dettivo config\n",
    "\n",
    "[dictation]\n",
    "# names I dictate\n",
    "vocabulary = [\n",
    "  \"Dettivo\", # the product\n",
    "  { term = \"Clef\", sounds_like = [\"clay\"] }, # the app\n",
    "]\n",
    "language = \"en\" # always\n",
    "\n",
    "[daemon]\n",
    "log_level = \"warn\"\n",
);

#[test]
fn an_import_adds_the_terms_and_keeps_the_files_comments_and_entries() {
    let d = daemon_with(HAND_WRITTEN);
    let list = list_file(
        &d,
        "names.txt",
        "# from the wiki\nOmarchy\n\nSapienXT = sapient x t, sapien xt\n",
    );
    let out = d.cli(&["vocabulary", "import", list.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "added 2 terms to dictation.vocabulary (4 entries)\n"
    );
    let imported = HAND_WRITTEN.replace(
        "  { term = \"Clef\", sounds_like = [\"clay\"] }, # the app\n",
        concat!(
            "  { term = \"Clef\", sounds_like = [\"clay\"] }, # the app\n",
            "  \"Omarchy\",\n",
            "  { term = \"SapienXT\", sounds_like = [\"sapient x t\", \"sapien xt\"] },\n",
        ),
    );
    assert_eq!(file(&d), imported);
    // The daemon took the write: what it now holds is the whole list.
    assert_eq!(
        vocabulary(&d),
        json!([
            "Dettivo",
            {"term": "Clef", "sounds_like": ["clay"]},
            "Omarchy",
            {"term": "SapienXT", "sounds_like": ["sapient x t", "sapien xt"]},
        ])
    );
    // The round trip: the same list again adds nothing and leaves every byte.
    let again = d.cli(&["vocabulary", "import", list.to_str().unwrap()]);
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert_eq!(
        stdout(&again),
        concat!(
            "nothing to add; dictation.vocabulary holds every term\n",
            "skipped 2 duplicates\n",
            "  line 2: \"Omarchy\" is already in the vocabulary\n",
            "  line 4: \"SapienXT\" is already in the vocabulary\n",
        )
    );
    assert_eq!(file(&d), imported);
}

#[test]
fn duplicates_are_reported_and_a_broken_limit_or_a_dry_run_writes_nothing() {
    let d = daemon_with(HAND_WRITTEN);
    let list = list_file(
        &d,
        "dupes.txt",
        "dettivo\nHyprland\nCLEF = cleff\nhyprland\n",
    );
    let dry = d.cli(&["vocabulary", "import", "--dry-run", list.to_str().unwrap()]);
    assert_eq!(dry.status.code(), Some(0), "{}", stderr(&dry));
    assert_eq!(
        stdout(&dry),
        concat!(
            "would add 1 term to dictation.vocabulary (3 entries after)\n",
            "  line 2: Hyprland\n",
            "skipped 3 duplicates\n",
            "  line 1: \"dettivo\" is already in the vocabulary\n",
            "  line 3: \"CLEF\" is already in the vocabulary\n",
            "  line 4: \"hyprland\" repeats line 2\n",
        )
    );
    assert_eq!(file(&d), HAND_WRITTEN, "a dry run writes nothing");

    let json_out = d.cli(&["--json", "vocabulary", "import", list.to_str().unwrap()]);
    assert_eq!(json_out.status.code(), Some(0), "{}", stderr(&json_out));
    let report: Value = serde_json::from_str(&stdout(&json_out)).unwrap();
    assert_eq!(report["written"], true);
    assert_eq!(report["added"], json!([{"line": 2, "entry": "Hyprland"}]));
    assert_eq!(report["skipped"].as_array().unwrap().len(), 3);
    assert_eq!(report["skipped"][2]["repeats_line"], 2);
    let written = file(&d);
    assert!(written.contains("  \"Hyprland\",\n"), "{written}");

    let long = "x".repeat(65);
    let spellings = vec!["s"; 17].join(", ");
    let broken = list_file(
        &d,
        "broken.txt",
        &format!("Niri\n{long}\nSway = {spellings}\n"),
    );
    let refused = d.cli(&["vocabulary", "import", broken.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(4), "invalid arguments");
    assert_eq!(
        stderr(&refused),
        format!(
            "dettivo: {}: nothing was written\n  line 2: {long:?} is 65 characters; a term holds at most 64\n  line 3: \"Sway\" has 17 sounds_like spellings; a term takes at most 16\n",
            broken.display()
        )
    );
    let full: String = (0..499).map(|i| format!("term{i}\n")).collect();
    let past = list_file(&d, "past.txt", &full);
    let refused = d.cli(&["vocabulary", "import", past.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(4));
    let err = stderr(&refused);
    assert!(
        err.contains(
            "  line 498: \"term497\" would be entry 501; a vocabulary holds at most 500\n"
        ) && err.contains("  line 499: \"term498\" would be entry 502;")
            && !err.contains("line 497:"),
        "{err}"
    );
    assert_eq!(file(&d), written, "a refused import writes nothing");
}

/// The bench's vocabulary (`set.json`) as `config get` returns it.
fn bench_vocabulary() -> Value {
    let set =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivo-qa/fixtures/vocabulary/set.json");
    let set: Value = serde_json::from_str(&std::fs::read_to_string(set).unwrap()).unwrap();
    set["vocabulary"].clone()
}

#[test]
fn the_bench_vocabulary_imported_from_a_file_is_the_one_written_by_hand() {
    // The term-recall bench (ADR 0087) primes every engine from the
    // vocabulary's entries alone, terms and spellings in order, so the
    // same entries give the same result.
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vocabulary.txt");
    let imported = daemon_with("[dictation]\nlanguage = \"en\"\n");
    let out = imported.cli(&["vocabulary", "import", fixture.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let entries = bench_vocabulary();
    let by_hand: Vec<String> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let spellings: Vec<String> = e["sounds_like"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| format!("{s}"))
                .collect();
            format!(
                "  {{ term = {}, sounds_like = [{}] }},\n",
                e["term"],
                spellings.join(", ")
            )
        })
        .collect();
    let hand = daemon_with(&format!(
        "[dictation]\nlanguage = \"en\"\nvocabulary = [\n{}]\n",
        by_hand.concat()
    ));
    assert_eq!(vocabulary(&imported), entries);
    assert_eq!(vocabulary(&imported), vocabulary(&hand));
}

/// The local Parakeet v2 model and the jfk fixture, when both exist.
fn local_parakeet() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/parakeet/parakeet-v2/tdt-0.6b-v2-q8_0.gguf");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

const V2_MANIFEST: &str = r#"{"provider":"parakeet","id":"parakeet-v2","sha256":"2027e2e1a4dc60ccdd8558f93b15e7c0db4ef8895b4e82e889f3a6275d8119c6","size_bytes":903835936,"downloaded_at":0,"catalogue_version":1,"verified":true}"#;

/// `dettivo --json call <method> <params>`, which must succeed.
fn call(d: &Daemon, method: &str, params: Value) -> Value {
    let params = params.to_string();
    let out = d.cli(&["--json", "--timeout-ms", "120000", "call", method, &params]);
    assert_eq!(out.status.code(), Some(0), "{method}: {}", stderr(&out));
    serde_json::from_str(&stdout(&out)).unwrap()
}

/// One dictation of the mock microphone's fixture; its end is the
/// release. The text of the `n`th take, once it is in the history.
fn take(d: &Daemon, n: usize) -> String {
    call(
        d,
        "dictation.start",
        json!({"language": "en", "mode": "raw"}),
    );
    let mut items = json!([]);
    let finished = wait_for(std::time::Duration::from_secs(120), || {
        items = call(
            d,
            "transcripts.list",
            json!({"kinds": ["dictation"], "limit": 10, "cursor": null}),
        )["items"]
            .clone();
        items.as_array().is_some_and(|i| i.len() >= n)
    });
    assert!(finished, "take {n} never finished");
    let newest = call(d, "transcripts.get", json!({"ref": items[0]["ref"]}));
    newest["text_polish"].as_str().unwrap().to_string()
}

fn wait_for(limit: std::time::Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

#[test]
fn the_running_daemon_uses_imported_terms_on_the_next_take() {
    let Some((model, wav)) = local_parakeet() else {
        eprintln!("skip: parakeet-v2 or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = Tree::new();
    let dir = tree.root().join("data/dettivo/models/parakeet/parakeet-v2");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&model, dir.join(model.file_name().unwrap())).unwrap();
    std::fs::write(dir.join("manifest.json"), V2_MANIFEST).unwrap();
    let bin_dir = Path::new(common::CLI).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-parakeet").is_file() {
        let status = std::process::Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-parakeet"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[engines.parakeet]\nbackend = \"cpu\"\n[speech]\nprovider = \"parakeet\"\nmodel = \"parakeet-v2\"\nparakeet_model_id = \"parakeet-v2\"\n[dictation]\nlanguage = \"en\"\nvocabulary_boost = \"strong\"\n",
        bin_dir.display()
    ));
    let d = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let pid = d.child.id();
    let before = take(&d, 1);
    assert!(before.contains("your country"), "{before}");

    // jfk.wav says "country"; the imported term is written over it only
    // where the vocabulary is in force.
    let list = list_file(
        &d,
        "terms.txt",
        "# a term the model never hears\nKuntry = country\n",
    );
    let out = d.cli(&["vocabulary", "import", list.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let after = take(&d, 2);
    assert!(after.contains("your Kuntry"), "{after}");
    assert_eq!(d.child.id(), pid, "the same daemon, never restarted");
}
