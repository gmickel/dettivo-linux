//! The verbs of `dettivo-qa` that need a daemon or a driver: `contract`,
//! `mcp`, `drive` and `audio-check`, with the daemon they start for the
//! replays.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use dettivo_qa::{audio, evidence, profile, replay, runner, scenarios};

use crate::{Cli, models_dir};
use dettivo_qa::contract::{REST_TOKEN, config_key, harness_with_test_model, rest_rows};

/// The clip the MCP harness imports live, when the test model and the
/// clip are both present (`scripts/models/fetch-test-model.sh`).
fn import_fixture() -> Option<PathBuf> {
    let models = models_dir()?;
    let clip = models.join("fixtures/jfk.wav");
    (models.join("whisper/tiny.en/ggml-tiny.en.bin").is_file() && clip.is_file()).then_some(clip)
}

/// The socket to test against: `socket` as given, or a daemon of our own
/// in a fresh profile named after the verb (`contract`, `mcp`) so its
/// messages say which verb started it, with the history seed on (the
/// transcripts fixtures and the MCP harness need its rows). The daemon
/// and the profile live as long as the returned guards.
fn daemon_socket(
    repo: &Path,
    socket: Option<&Path>,
    verb: &str,
) -> Result<(PathBuf, Option<Killer>, Option<profile::Profile>), u8> {
    if let Some(s) = socket {
        return Ok((s.to_path_buf(), None, None));
    }
    let daemon_bin = scenarios::binary(repo, "dettivod").map_err(|e| {
        eprintln!("{verb}: {e}");
        2
    })?;
    // The replay's `speech.models.delete` fixtures delete what they find,
    // so the daemon sees the profile's private tree with the test model
    // hard-linked in, never the real model cache.
    let source = models_dir().map(|real| profile::ModelSource::new(real, repo));
    let profile = profile::Profile::create(verb, source.as_ref()).map_err(|e| {
        eprintln!("{verb}: profile: {e}");
        1
    })?;
    // The runner's daemon hosts the REST shim on an ephemeral port with a
    // known token (ADR 0028), so the REST fixtures replay beside the
    // socket ones; the port comes back in system.capabilities.rest.
    let config = profile.root.join("cfg/dettivo/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap_or_default();
    text.push_str("\n[rest]\nenabled = true\nport = 0\n");
    let _ = std::fs::write(&config, text);
    let child = profile::command(&daemon_bin, &profile.env())
        .env("DETTIVO_E2E_SEED", "1")
        .env("DETTIVO_IPC_TOKEN", REST_TOKEN)
        .env("DETTIVO_MOCK_A11Y", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            eprintln!("{verb}: start dettivod: {e}");
            2
        })?;
    let killer = Killer(child);
    let socket = profile.socket();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    // The socket is bound before the daemon answers; readiness is a ping
    // that comes back.
    while !replay::answers_ping(&socket) {
        if std::time::Instant::now() > deadline {
            eprintln!("{verb}: dettivod did not answer on {}", socket.display());
            return Err(2);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok((socket, Some(killer), Some(profile)))
}

pub(crate) fn rest(cli: &Cli, repo: &Path, socket: Option<&Path>) -> u8 {
    let own_daemon = socket.is_none();
    let (socket, _daemon, _profile) = match daemon_socket(repo, socket, "rest") {
        Ok(s) => s,
        Err(code) => return code,
    };
    let fixture = import_fixture();
    let selected =
        fixture.is_some() && own_daemon && config_key(&socket, "speech.model", Some("tiny.en"));
    let rows = rest_rows(repo, &socket, fixture.as_deref());
    if selected {
        config_key(&socket, "speech.model", None);
    }
    if rows.is_empty() {
        eprintln!(
            "rest: the daemon on {} hosts no REST listener",
            socket.display()
        );
        return 2;
    }
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).unwrap_or_default()
        );
    } else {
        print!("{}", dettivo_rest::harness::human(&rows));
    }
    u8::from(
        rows.iter()
            .any(|r| r.verdict == dettivo_rest::harness::Verdict::Fail),
    )
}

pub(crate) fn contract(cli: &Cli, repo: &Path, socket: Option<&Path>, strict: bool) -> u8 {
    let report = match socket {
        Some(socket) => replay::run(repo, socket).map_err(|e| e.to_string()),
        None => dettivo_qa::contract::run(repo, models_dir(), Duration::from_secs(10)),
    };
    match report {
        Ok(report) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                print!("{}", report.human());
            }
            u8::from(!report.ok(strict))
        }
        Err(e) => {
            eprintln!("contract: {e}");
            2
        }
    }
}

pub(crate) fn mcp(cli: &Cli, repo: &Path, socket: Option<&Path>, server: Option<&Path>) -> u8 {
    let server = match server {
        Some(s) => s.to_path_buf(),
        None => match scenarios::binary(repo, "dettivo-mcp") {
            Ok(b) => b,
            Err(e) => {
                eprintln!("mcp: {e}");
                return 2;
            }
        },
    };
    let own_daemon = socket.is_none();
    let (socket, _daemon, _profile) = match daemon_socket(repo, socket, "mcp") {
        Ok(s) => s,
        Err(code) => return code,
    };
    let fixture = import_fixture();
    let rows = harness_with_test_model(&server, &socket, fixture.as_deref(), own_daemon);
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).unwrap_or_default()
        );
    } else {
        print!("{}", dettivo_mcp::harness::human(&rows));
    }
    u8::from(
        rows.iter()
            .any(|r| r.verdict == dettivo_mcp::harness::Verdict::Fail),
    )
}

/// Kills a child on drop.
struct Killer(std::process::Child);

impl Drop for Killer {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .output();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while self.0.try_wait().ok().flatten().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(crate) fn drive(cli: &Cli, repo: &Path, scenario: &str, opts: runner::Options) -> u8 {
    let pack: Vec<Box<dyn scenarios::Scenario>> = if scenario == "all" {
        scenarios::all()
    } else {
        match scenarios::by_id(scenario) {
            Some(s) => vec![s],
            None => {
                eprintln!("drive: unknown scenario {scenario:?}; `dettivo-qa list` names the pack");
                return 4;
            }
        }
    };
    let mut receipts = Vec::new();
    let mut failed = false;
    for s in pack {
        match runner::drive(repo, s.as_ref(), &opts) {
            Ok(r) => {
                if !cli.json {
                    println!(
                        "{:<5} {:<24} {} {}ms{}",
                        format!("{:?}", r.outcome).to_lowercase(),
                        r.scenario,
                        r.driver,
                        r.duration_ms,
                        r.reason
                            .as_deref()
                            .map(|x| format!("  {x}"))
                            .unwrap_or_default()
                    );
                }
                failed |= r.outcome == evidence::Outcome::Fail;
                receipts.push(r);
            }
            Err(e) => {
                eprintln!("drive {}: {e}", s.id());
                failed = true;
            }
        }
    }
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&receipts).unwrap_or_default()
        );
    }
    u8::from(failed)
}

pub(crate) fn audio_check(cli: &Cli, repo: &Path, tolerance: f64) -> u8 {
    let work = match tempfile::Builder::new()
        .prefix("dqaudio-")
        .tempdir_in("/tmp")
    {
        Ok(d) => d,
        Err(e) => {
            eprintln!("audio-check: {e}");
            return 1;
        }
    };
    match audio::check(repo, work.path(), tolerance) {
        Ok(report) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                println!(
                    "audio-check: {} sink={} monitor={} correlation={} unload_detected={}{}",
                    report.outcome,
                    report.sink,
                    report.monitor.as_deref().unwrap_or("-"),
                    report
                        .correlation
                        .map(|c| format!("{c:.3}"))
                        .unwrap_or_else(|| "-".into()),
                    report
                        .unload_detected
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "-".into()),
                    report
                        .reason
                        .as_deref()
                        .map(|r| format!("  {r}"))
                        .unwrap_or_default()
                );
            }
            u8::from(report.outcome == "fail")
        }
        Err(e) => {
            eprintln!("audio-check: {e}");
            1
        }
    }
}
