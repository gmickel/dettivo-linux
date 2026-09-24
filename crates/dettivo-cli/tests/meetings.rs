//! R5 for the CLI: the notes, analysis, export, delete and disclosure
//! verbs against a seeded daemon whose language model is the QA fixture
//! and whose insertion chain is the mock. `--json` prints the daemon's
//! result raw (the snapshots the fixtures pin), the export lands byte-
//! equal to the storage goldens, and `disclosure --copy` puts the macOS
//! text in the mock sink.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::*;
use serde_json::Value;

const RICH: &str = "5eed0000-0000-4000-8000-00000000a001";
const NOTES_ONLY: &str = "5eed0000-0000-4000-8000-00000000a003";
const ANSWER: &str = r#"{"summary": "Agreed the Q2 budget.", "decisions": ["Hire two engineers"], "action_items": [{"text": "Write the runbook", "owner": "Gordon", "due": "Wednesday"}]}"#;

fn fixture(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-proto/fixtures")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-storage/tests/goldens")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn notes_analysis_export_delete_and_the_disclosure_copy_work_end_to_end() {
    let llm = tempfile::Builder::new()
        .prefix("dtvllm")
        .tempdir_in("/tmp")
        .unwrap();
    std::fs::write(llm.path().join("default.txt"), ANSWER).unwrap();
    let mock_llm = format!("fixture:{}", llm.path().display());
    let d = Daemon::spawn(
        Tree::new(),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DETTIVO_MOCK_LLM", &mock_llm),
        ],
    );

    // notes get prints the Markdown; --json the raw result (the fixture).
    let notes = d.cli(&["meetings", "notes", "get", RICH]);
    assert_eq!(notes.status.code(), Some(0), "{}", stderr(&notes));
    assert!(stdout(&notes).starts_with("# Roadmap review\n\n- Budget for Q2 agreed"));
    let raw: Value = serde_json::from_str(&stdout(
        &d.cli(&["--json", "meetings", "notes", "get", RICH]),
    ))
    .unwrap();
    assert_eq!(
        raw,
        fixture("meetings/notes.get.json")["response"]["result"]
    );

    // notes set from the argument and from a file.
    let set = d.cli(&[
        "meetings",
        "notes",
        "set",
        NOTES_ONLY,
        "Ask for pricing.",
        "--source",
        "live",
    ]);
    assert_eq!(set.status.code(), Some(0), "{}", stderr(&set));
    assert_eq!(stdout(&set), "notes stored (16 bytes, live)\n");
    let file = d.tree.root().join("notes-in.md");
    std::fs::write(&file, "# From a file\n").unwrap();
    let set = d.cli(&[
        "--json",
        "meetings",
        "notes",
        "set",
        NOTES_ONLY,
        "--file",
        file.to_str().unwrap(),
    ]);
    let stored: Value = serde_json::from_str(&stdout(&set)).unwrap();
    assert_eq!(stored["markdown"], "# From a file\n");
    assert_eq!(stored["source"], "user");
    let none = d.cli(&["meetings", "notes", "set", NOTES_ONLY]);
    assert_eq!(none.status.code(), Some(4));

    // analysis prints the seeded analysis; --json is the fixture.
    let shown = stdout(&d.cli(&["meetings", "analysis", RICH]));
    assert!(
        shown.starts_with("status: ready\nmodel: qwen3-4b-instruct-2507\n"),
        "{shown}"
    );
    assert!(shown.contains("- Write the rollout runbook (owner: Gordon, due: Wednesday)\n"));
    let raw: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "meetings", "analysis", RICH]))).unwrap();
    assert_eq!(
        raw,
        fixture("meetings/analysis.get.json")["response"]["result"]
    );

    // The exports land byte-equal to the storage goldens.
    for format in ["txt", "md", "srt", "vtt", "json"] {
        let out = d.tree.root().join(format!("meeting.{format}"));
        let exported = d.cli(&[
            "meetings",
            "export",
            RICH,
            "--format",
            format,
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_eq!(exported.status.code(), Some(0), "{}", stderr(&exported));
        assert!(
            stdout(&exported).starts_with("wrote "),
            "{}",
            stdout(&exported)
        );
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            golden(&format!("meeting.{format}")),
            "{format}"
        );
    }
    let raw_out = d.tree.root().join("raw.txt");
    d.cli(&[
        "meetings",
        "export",
        RICH,
        "--format",
        "txt",
        "--raw",
        "--out",
        raw_out.to_str().unwrap(),
    ]);
    assert!(
        std::fs::read_to_string(&raw_out)
            .unwrap()
            .contains("um okay so the roadmap")
    );
    let bad = d.cli(&[
        "meetings", "export", RICH, "--format", "docx", "--out", "/tmp/x",
    ]);
    assert_eq!(bad.status.code(), Some(4));
    assert!(
        stderr(&bad).contains("txt, md, srt, vtt, json"),
        "{}",
        stderr(&bad)
    );

    // analyze --force runs the fixture; the result shows once ready.
    let started: Value = serde_json::from_str(&stdout(
        &d.cli(&["--json", "meetings", "analyze", RICH, "--force"]),
    ))
    .unwrap();
    assert_eq!(started["job"]["state"], "running", "{started}");
    let deadline = Instant::now() + Duration::from_secs(20);
    let shown = loop {
        let shown = stdout(&d.cli(&["meetings", "analysis", RICH]));
        if shown.starts_with("status: ready") && shown.contains("Agreed the Q2 budget.") {
            break shown;
        }
        assert!(Instant::now() < deadline, "{shown}");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(shown.contains("- Hire two engineers\n"), "{shown}");
    let idle = stdout(&d.cli(&["meetings", "analyze", RICH]));
    assert_eq!(idle, "analysis ready (job_analysis)\n");

    // disclosure --copy lands the macOS text in the mock sink.
    let copied = d.cli(&["meetings", "disclosure", "--copy"]);
    assert_eq!(copied.status.code(), Some(0), "{}", stderr(&copied));
    assert_eq!(
        stdout(&copied),
        "disclosure message copied to the clipboard\n"
    );
    let sink =
        std::fs::read_to_string(d.tree.root().join("state/dettivo/qa/inserted.txt")).unwrap();
    assert_eq!(
        sink.trim_end(),
        fixture("meetings/disclosure.get.json")["response"]["result"]["message"]
            .as_str()
            .unwrap()
    );
    let both = d.cli(&["meetings", "disclosure", "--copy", "--acknowledge"]);
    assert_eq!(both.status.code(), Some(4));

    // delete takes the configured policy, or one by name.
    let kept = d.cli(&[
        "--json",
        "meetings",
        "delete",
        NOTES_ONLY,
        "--policy",
        "transcript_only",
    ]);
    assert_eq!(kept.status.code(), Some(0), "{}", stderr(&kept));
    let row: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "meetings", "get", NOTES_ONLY]))).unwrap();
    assert_eq!(row["transcript"], "");
    assert_eq!(row["notes"], "# From a file\n");
    let gone = d.cli(&["meetings", "delete", NOTES_ONLY]);
    assert_eq!(gone.status.code(), Some(0), "{}", stderr(&gone));
    let missing = d.cli(&["meetings", "get", NOTES_ONLY]);
    assert_eq!(missing.status.code(), Some(1));
}

/// agent-surfaces/F1 (fn-43): `dettivo meetings delete <id>` names no
/// policy, so the daemon applies `[meetings] delete_artifact_policy`;
/// the CLI never reads the key, never decides `all` on its own.
#[test]
fn a_delete_without_a_policy_leaves_the_policy_to_the_daemon() {
    let tree = Tree::new();
    tree.write_config("[meetings]\ndelete_artifact_policy = \"transcript_only\"\n");
    let d = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")]);
    let deleted = d.cli(&["--json", "meetings", "delete", RICH]);
    assert_eq!(deleted.status.code(), Some(0), "{}", stderr(&deleted));
    let row: Value =
        serde_json::from_str(&stdout(&d.cli(&["--json", "meetings", "get", RICH]))).unwrap();
    assert_eq!(
        row["title"], "Roadmap review",
        "the configured policy kept the row"
    );
    assert_eq!(row["transcript"], "");
}
