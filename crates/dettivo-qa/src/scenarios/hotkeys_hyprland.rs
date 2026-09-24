//! Hotkeys on Hyprland (R2): the snippet `dettivo setup hyprland` renders
//! is loaded into the running compositor with the scenario's own chords
//! and a `dettivo` that talks to the scenario's daemon, a virtual keyboard
//! presses the chords, and the daemon's event stream shows every
//! transition: hold starts and the release stops, toggle starts and stops,
//! cancel cancels. Skipped with the reason when `hyprctl`, a Hyprland
//! session or a writable `/dev/uinput` is missing.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use evdev::KeyCode;
use serde_json::{Value, json};

use super::hotkeys_hyprland_chords::Chords;
use super::support::installed;
use super::{Context, Scenario, binary};
use crate::driver::Driver;
use crate::keys::{self, Keyboard};

/// The scenario.
pub struct HotkeysHyprland;

/// The modifier keys of every chord.
const MODIFIERS: &[KeyCode] = &[
    KeyCode::KEY_LEFTCTRL,
    KeyCode::KEY_LEFTALT,
    KeyCode::KEY_LEFTSHIFT,
];

fn hyprctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("hyprctl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("hyprctl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || text.starts_with("error") {
        return Err(format!(
            "hyprctl {}: {text}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(text)
}

/// Collects `dictation.state` states from a subscription until dropped.
struct States {
    seen: Arc<Mutex<Vec<Value>>>,
}

impl States {
    fn subscribe(socket: &std::path::Path) -> Result<Self, String> {
        let mut stream = UnixStream::connect(socket).map_err(|e| format!("subscribe: {e}"))?;
        let req = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": ["dictation.state"], "buffer": 256}});
        stream
            .write_all(format!("{req}\n").as_bytes())
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                if let Ok(v) = serde_json::from_str::<Value>(line.trim_end()) {
                    if v["method"] == "events.notify" {
                        sink.lock().unwrap().push(v["params"].clone());
                    }
                }
                line.clear();
            }
        });
        Ok(Self { seen })
    }

    fn states(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter_map(|p| p["payload"]["state"].as_str().map(str::to_string))
            .collect()
    }

    fn wait_for(&self, count: usize, last: &str, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let states = self.states();
            if states.len() >= count && states.last().is_some_and(|s| s == last) {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "waited for {count} states ending in {last:?}, saw {states:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn jfk(ctx: &Context<'_>) -> Result<std::path::PathBuf, String> {
    let wav = ctx
        .profile
        .root
        .join("data/dettivo/models/fixtures/jfk.wav");
    wav.is_file().then_some(wav).ok_or_else(|| {
        "jfk.wav is not under the model directory (scripts/models/fetch-test-model.sh)".to_string()
    })
}

impl Scenario for HotkeysHyprland {
    fn id(&self) -> &'static str {
        "hotkeys_hyprland"
    }

    fn summary(&self) -> &'static str {
        "the Hyprland snippet drives hold, toggle and cancel through real key presses"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preflight(&self) -> Result<(), String> {
        if !installed("hyprctl") {
            return Err("hyprctl is not on PATH".into());
        }
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return Err("not a Hyprland session (HYPRLAND_INSTANCE_SIGNATURE is unset)".into());
        }
        hyprctl(&["eval", "return 1"])
            .map(|_| ())
            .map_err(|_| "this Hyprland has no Lua config (hyprctl eval failed)".to_string())?;
        keys::available()
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let wav = jfk(ctx)?;
        let daemon_bin = binary(ctx.repo_root, "dettivod")?;
        let cli = binary(ctx.repo_root, "dettivo")?;
        let model = ctx
            .profile
            .root
            .join("data/dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
        if !model.is_file() {
            return Err("tiny.en is not under the model directory".into());
        }
        let engines = daemon_bin
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        // The chords are reserved from what the compositor holds right
        // now, so a binding of the user's on the same chord is never
        // unbound by the snippet or by the cleanup.
        let chords = Chords::reserve(&hyprctl(&["binds", "-j"])?)?;
        std::fs::write(
            ctx.profile.root.join("cfg/dettivo/config.toml"),
            format!(
                "[engines]\ndirectory = \"{engines}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n{}",
                chords.config()
            ),
        )
        .map_err(|e| e.to_string())?;
        let mut env: BTreeMap<String, String> = ctx.profile.env();
        env.insert("DETTIVO_MOCK_MIC".into(), wav.display().to_string());
        env.insert("RUST_LOG".into(), "info".into());
        let log = std::fs::File::create(ctx.evidence_dir.join("daemon.log"))
            .map_err(|e| format!("daemon.log: {e}"))?;
        ctx.evidence.push("daemon.log".into());
        let child = crate::profile::command(&daemon_bin, &env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map(ChildOwner::new)
            .map_err(|e| format!("start dettivod: {e}"))?;
        ctx.profile.track("dettivod", &child);
        let socket = ctx.profile.socket();
        let deadline = Instant::now() + ctx.timeout;
        while !crate::replay::answers_ping(&socket) {
            if Instant::now() > deadline {
                return Err(format!("dettivod did not answer on {}", socket.display()));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        ctx.timings.mark("daemon");

        // The snippet as `dettivo setup` renders it, pointed at this daemon.
        let rendered = crate::profile::command(&cli, &env)
            .args(["setup", "hyprland-lua", "--stdout"])
            .output()
            .map_err(|e| format!("dettivo setup: {e}"))?;
        if !rendered.status.success() {
            return Err(format!(
                "dettivo setup failed: {}",
                String::from_utf8_lossy(&rendered.stderr)
            ));
        }
        let snippet = String::from_utf8_lossy(&rendered.stdout).replace(
            "dettivo --quiet dictation",
            &format!(
                "{} --socket {} --quiet dictation",
                cli.display(),
                socket.display()
            ),
        );
        std::fs::write(ctx.evidence_dir.join("snippet.lua"), &snippet)
            .map_err(|e| e.to_string())?;
        ctx.evidence.push("snippet.lua".into());
        let events = States::subscribe(&socket)?;
        let outcome = drive_keys(&snippet, &chords, &events);
        // The chords come back out whatever the drive did, and a chord
        // that stays bound is a failure of its own.
        let restored = hyprctl(&["eval", &chords.unbind()])
            .and_then(|_| hyprctl(&["binds", "-j"]))
            .and_then(|binds| chords.still_bound(&binds))
            .and_then(|left| {
                if left.is_empty() {
                    Ok(())
                } else {
                    Err(format!("chords still bound after the cleanup: F{left:?}"))
                }
            });
        let outcome = match (outcome, restored) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(r)) => Err(r),
            (Err(e), Ok(())) => Err(e),
            (Err(e), Err(r)) => Err(format!("{e}; and {r}")),
        };
        let lines: Vec<String> = events
            .seen
            .lock()
            .unwrap()
            .iter()
            .map(|p| p.to_string())
            .collect();
        std::fs::write(
            ctx.evidence_dir.join("events.jsonl"),
            lines.join("\n") + "\n",
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("events.jsonl".into());
        ctx.timings.mark("keys");
        outcome
    }
}

/// The snippet as one `hyprctl eval` argument: comment lines dropped
/// (hyprctl reads a leading `--` as one of its own flags), the rest on
/// one line.
fn eval_source(snippet: &str) -> String {
    snippet
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Loads the snippet, presses the chords and checks the transitions.
fn drive_keys(snippet: &str, chords: &Chords, events: &States) -> Result<(), String> {
    hyprctl(&["eval", &eval_source(snippet)])?;
    let binds = hyprctl(&["binds", "-j"])?;
    if chords.still_bound(&binds)?.len() != 4 {
        return Err(format!(
            "Hyprland did not register every chord: F{:?} bound of F{:?}",
            chords.still_bound(&binds)?,
            chords.keys()
        ));
    }
    let (hold, toggle, cancel) = (
        Chords::code(chords.hold),
        Chords::code(chords.toggle),
        Chords::code(chords.cancel),
    );
    let mut keyboard = Keyboard::new(&[])?;
    let long = Duration::from_secs(60);
    // Hold to talk: press starts, release stops.
    keyboard.chord_down(MODIFIERS, hold)?;
    events.wait_for(1, "recording", Duration::from_secs(15))?;
    std::thread::sleep(Duration::from_millis(2500));
    keyboard.chord_up(MODIFIERS, hold)?;
    events.wait_for(4, "idle", long)?;
    // Toggle: one press starts, the next stops.
    keyboard.chord(MODIFIERS, toggle)?;
    events.wait_for(5, "recording", Duration::from_secs(15))?;
    std::thread::sleep(Duration::from_millis(1500));
    keyboard.chord(MODIFIERS, toggle)?;
    events.wait_for(8, "idle", long)?;
    // Cancel drops the session.
    keyboard.chord(MODIFIERS, toggle)?;
    events.wait_for(9, "recording", Duration::from_secs(15))?;
    keyboard.chord(MODIFIERS, cancel)?;
    events.wait_for(11, "idle", Duration::from_secs(15))?;
    let states = events.states();
    let expected = [
        "recording",
        "transcribing",
        "inserting",
        "idle",
        "recording",
        "transcribing",
        "inserting",
        "idle",
        "recording",
        "cancelled",
        "idle",
    ];
    if states != expected {
        return Err(format!("transitions {states:?}, expected {expected:?}"));
    }
    Ok(())
}
