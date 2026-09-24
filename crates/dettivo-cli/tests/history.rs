//! R5 and R6 on the command line: `dettivo history` verbs against a seeded
//! daemon, `export --out` matching the goldens, an archive round trip
//! through `import`, the doctor's history row, and with the local tiny.en
//! an audio import that follows its job's progress, `get --words` and
//! `cancel`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

const CLI: &str = env!("CARGO_BIN_EXE_dettivo");
const SAMPLE: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

fn daemon_binary() -> PathBuf {
    let candidate = Path::new(CLI).parent().unwrap().join("dettivod");
    if !candidate.is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "dettivod"])
            .status()
            .expect("cargo build -p dettivod");
        assert!(status.success());
    }
    candidate
}

struct Daemon {
    dir: TempDir,
    child: Child,
}

impl Daemon {
    fn spawn(seed: bool) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("dth")
            .tempdir_in("/tmp")
            .unwrap();
        Self::spawn_in(dir, seed)
    }

    fn spawn_in(dir: TempDir, seed: bool) -> Self {
        let mut cmd = Command::new(daemon_binary());
        Self::env(dir.path(), &mut cmd);
        cmd.env("DETTIVO_QA_MODE", "1")
            .env("DETTIVO_MOCK_INSERT", "1");
        if seed {
            cmd.env("DETTIVO_E2E_SEED", "1");
        }
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let socket = dir.path().join("run/dettivo/dettivo.sock");
        let start = Instant::now();
        loop {
            if let Ok(mut s) = UnixStream::connect(&socket) {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                if s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":\"r\",\"method\":\"system.ping\",\"params\":{}}\n").is_ok() {
                    let mut line = String::new();
                    if BufReader::new(s).read_line(&mut line).is_ok_and(|n| n > 0) {
                        break;
                    }
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "daemon not ready"
            );
            thread::sleep(Duration::from_millis(10));
        }
        Self { dir, child }
    }

    fn env(root: &Path, cmd: &mut Command) {
        for var in [
            "DETTIVO_CONFIG",
            "DETTIVO_IPC_SOCKET",
            "DETTIVO_IPC_TOKEN",
            "DETTIVO_DATA_DIR",
            "DETTIVO_QA",
        ] {
            cmd.env_remove(var);
        }
        cmd.env("HOME", root)
            .env("XDG_CONFIG_HOME", root.join("cfg"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_RUNTIME_DIR", root.join("run"));
    }

    fn cli(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(CLI);
        Self::env(self.dir.path(), &mut cmd);
        cmd.args(args).output().unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// A JSON export with every `audio_path` levelled to null: the seed keeps
/// one take beside the profile's database (`docs/history.md`), and its
/// path is the profile's, not the golden's. Other formats pass through.
fn without_audio_paths(text: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_string();
    };
    if let Some(items) = value["items"].as_array_mut() {
        for item in items.iter_mut() {
            if item.get("audio_path").is_some() {
                item["audio_path"] = serde_json::Value::Null;
            }
        }
    }
    serde_json::to_string_pretty(&value).unwrap() + "\n"
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dettivo-storage/tests/goldens")
            .join(name),
    )
    .unwrap()
}

#[test]
fn history_verbs_list_get_latest_search_and_delete_over_the_seed() {
    let d = Daemon::spawn(true);
    let list = d.cli(&["history", "list", "--limit", "3"]);
    assert_eq!(
        list.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let listed = out(&list);
    let lines: Vec<&str> = listed.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with(SAMPLE), "{}", lines[0]);
    assert!(
        lines[0].contains("2026-02-13T16:00:00Z")
            && lines[0].contains("completed")
            && lines[0].ends_with("Dictation")
    );

    let by_app = d.cli(&[
        "--json",
        "history",
        "list",
        "--app",
        "com.mitchellh.ghostty",
        "--since",
        "2026-02-12",
    ]);
    let rows: Value = serde_json::from_str(&out(&by_app)).unwrap();
    assert_eq!(rows["items"].as_array().unwrap().len(), 3);

    let latest = d.cli(&["history", "latest"]);
    assert_eq!(
        out(&latest),
        format!("{SAMPLE}\nThe contract sample dictation. Thirty seconds of speech.\n")
    );
    let get = d.cli(&["history", "get", "5eed0000-0000-4000-8000-000000000006"]);
    assert_eq!(out(&get), "Run the benchmark on the Vulkan backend.\n");
    let get_json: Value =
        serde_json::from_str(&out(&d.cli(&["--json", "history", "get", SAMPLE]))).unwrap();
    assert_eq!(
        get_json["text_raw"],
        "the contract sample dictation period thirty seconds of speech"
    );

    let search = d.cli(&["history", "search", "api"]);
    assert_eq!(out(&search).lines().count(), 2, "{}", out(&search));
    let none = d.cli(&["history", "search", "zebra"]);
    assert_eq!(out(&none), "no matches\n");
    let long = d.cli(&["history", "search", &"x".repeat(2000)]);
    assert_eq!(long.status.code(), Some(4));

    let delete = d.cli(&["history", "delete", "5eed0000-0000-4000-8000-000000000006"]);
    assert_eq!(
        out(&delete),
        "deleted 5eed0000-0000-4000-8000-000000000006\n"
    );
    let gone = d.cli(&["history", "get", "5eed0000-0000-4000-8000-000000000006"]);
    assert_eq!(gone.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&gone.stderr).contains("NOT_FOUND"));
    let rerun = d.cli(&["history", "rerun", SAMPLE, "--model", "tiny.en"]);
    assert_eq!(rerun.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rerun.stderr).contains("no retained audio"));

    let doctor = d.cli(&["doctor"]);
    let history_row = out(&doctor)
        .lines()
        .find(|l| l.starts_with("history"))
        .map(str::to_string)
        .unwrap_or_default();
    assert!(
        history_row.contains("11 items")
            && history_row.contains("dettivo.db")
            && history_row.contains("0008-analysis-queued"),
        "{history_row}"
    );
    let doctor_json: Value = serde_json::from_str(&out(&d.cli(&["--json", "doctor"]))).unwrap();
    assert_eq!(doctor_json["history"]["item_count"], 11);
    assert_eq!(doctor_json["history"]["schema_version"], 8);
}

#[test]
fn export_matches_the_goldens_and_an_archive_imports_into_an_empty_profile() {
    let d = Daemon::spawn(true);
    let scratch = tempfile::tempdir().unwrap();
    for (format, name) in [
        ("json", "seed.json"),
        ("markdown", "seed.md"),
        ("txt", "seed.txt"),
    ] {
        let file = scratch.path().join(name);
        let export = d.cli(&[
            "history",
            "export",
            "--format",
            format,
            "--out",
            file.to_str().unwrap(),
        ]);
        assert_eq!(
            export.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&export.stderr)
        );
        assert!(out(&export).starts_with(&format!("wrote {}", file.display())));
        assert_eq!(
            without_audio_paths(&std::fs::read_to_string(&file).unwrap()),
            golden(name),
            "{format}"
        );
    }
    let one = scratch.path().join("one.json");
    d.cli(&[
        "history",
        "export",
        "--scope",
        "item",
        "--id",
        SAMPLE,
        "--format",
        "json",
        "--out",
        one.to_str().unwrap(),
    ]);
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&one).unwrap()).unwrap();
    assert_eq!(doc["items"].as_array().unwrap().len(), 1);
    let bad = d.cli(&[
        "history",
        "export",
        "--scope",
        "item",
        "--out",
        one.to_str().unwrap(),
    ]);
    assert_eq!(bad.status.code(), Some(4));
    let bad_format = d.cli(&[
        "history",
        "export",
        "--format",
        "srt",
        "--out",
        one.to_str().unwrap(),
    ]);
    assert_eq!(bad_format.status.code(), Some(4));

    let zip = scratch.path().join("dictations.zip");
    let export = d.cli(&[
        "history",
        "export",
        "--format",
        "zip",
        "--out",
        zip.to_str().unwrap(),
    ]);
    assert_eq!(export.status.code(), Some(0));
    drop(d);

    let empty = Daemon::spawn(false);
    let none = empty.cli(&["history", "list"]);
    assert_eq!(out(&none), "no dictations yet\n");
    let import = empty.cli(&["history", "import", zip.to_str().unwrap()]);
    assert_eq!(
        import.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&import.stderr)
    );
    assert!(
        out(&import).contains("12 items restored"),
        "{}",
        out(&import)
    );
    let listed: Value = serde_json::from_str(&out(
        &empty.cli(&["--json", "history", "list", "--limit", "50"])
    ))
    .unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 12);
    assert_eq!(listed["items"][0]["ref"]["id"], SAMPLE);
    let unsupported = empty.cli(&["history", "import", one.to_str().unwrap()]);
    assert_eq!(unsupported.status.code(), Some(4));
}

/// The local test model and clip, when present.
fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

/// A daemon with tiny.en selected and the engine beside the binaries.
fn daemon_with_tiny(model: &Path) -> Daemon {
    let dir = tempfile::Builder::new()
        .prefix("dth")
        .tempdir_in("/tmp")
        .unwrap();
    let models = dir.path().join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&models).unwrap();
    std::os::unix::fs::symlink(model, models.join("ggml-tiny.en.bin")).unwrap();
    let bin_dir = Path::new(CLI).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    let cfg = dir.path().join("cfg/dettivo/config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &cfg,
        format!(
            "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n",
            bin_dir.display()
        ),
    )
    .unwrap();
    Daemon::spawn_in(dir, false)
}

#[test]
fn import_follows_the_job_and_get_words_prints_the_segments() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let d = daemon_with_tiny(&model);
    let import = d.cli(&[
        "history",
        "import",
        wav.to_str().unwrap(),
        "--model",
        "tiny.en",
    ]);
    let stderr = String::from_utf8_lossy(&import.stderr);
    assert_eq!(import.status.code(), Some(0), "{stderr}");
    let stdout = out(&import);
    let mut lines = stdout.lines();
    let first = lines.next().unwrap_or("");
    assert!(first.ends_with(" completed"), "{stdout}");
    let id = first.split_whitespace().next().unwrap().to_string();
    assert!(stdout.to_lowercase().contains("americans"), "{stdout}");
    assert!(
        stderr.contains("job_import_1 decoding")
            && stderr.contains("job_import_1 transcribing chunk 1/1")
            && stderr.contains("job_import_1 merging")
            && stderr.contains("job_import_1 done"),
        "{stderr}"
    );
    let words = d.cli(&["history", "get", &id, "--words"]);
    let listing = out(&words);
    assert!(listing.starts_with("[00:00."), "{listing}");
    assert!(listing.to_lowercase().contains("americans"), "{listing}");
    let plain = out(&d.cli(&["history", "get", &id]));
    assert!(!plain.starts_with('['), "{plain}");
    let cancel = d.cli(&["history", "cancel", &id]);
    assert_eq!(cancel.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&cancel.stderr).contains("no job is working"));
    let missing = d.cli(&[
        "history",
        "import",
        wav.to_str().unwrap(),
        "--model",
        "base",
    ]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("dettivo speech download --model base")
    );
    let quiet = d.cli(&[
        "--json",
        "history",
        "import",
        wav.to_str().unwrap(),
        "--no-wait",
    ]);
    let doc: Value = serde_json::from_str(&out(&quiet)).unwrap();
    assert_eq!(doc["job"]["state"], "running");
    assert!(String::from_utf8_lossy(&quiet.stderr).is_empty());
}
