//! R1 with a model: the Qwen3 1.7B catalogue file under the daemon's model
//! layout (`scripts/models/fetch-llm-test-model.sh`), skipped by name when
//! it is absent. CLI mode prints the protocol's `generate` JSON; the
//! protocol streams `partial` events, answers `status` busy meanwhile,
//! stops on `cancel` between two tokens with `finish_reason = cancelled`,
//! and a corrupt model is `load_failed` naming the path and the reason.

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, read_frame, write_frame};
use serde_json::{Value, json};

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-llm");
const SYSTEM: &str = "You rewrite dictated text. Answer with the rewritten text only.";
const USER: &str = "um so i think we should ship this thing tomorrow";

/// The backend every model-backed route in this suite runs on: the CPU,
/// or Vulkan when `DETTIVO_TEST_BACKEND=vulkan` (the `test-llm-vulkan`
/// recipe sets it on a Vulkan build), in which case a load that does not
/// land on the device fails the test instead of running on the CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Cpu,
    Vulkan,
}

impl Backend {
    fn from_env() -> Self {
        match std::env::var("DETTIVO_TEST_BACKEND").as_deref() {
            Ok("vulkan") => Self::Vulkan,
            _ => Self::Cpu,
        }
    }

    /// The `backend` the results must report.
    fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Vulkan => "vulkan",
        }
    }

    /// The CLI flags that pin it (`--cpu`, or a strict `--backend vulkan`).
    fn cli_args(self) -> &'static [&'static str] {
        match self {
            Self::Cpu => &["--cpu"],
            Self::Vulkan => &["--backend", "vulkan"],
        }
    }
}

fn test_model() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let p = data.join("dettivo/models/llm/qwen3-1.7b/Qwen3-1.7B-Q4_K_M.gguf");
    p.is_file().then_some(p)
}

fn skip() -> Option<PathBuf> {
    let m = test_model();
    if m.is_none() {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
    }
    m
}

#[test]
fn cli_mode_prints_the_generate_json_the_protocol_carries() {
    let Some(model) = skip() else { return };
    let backend = Backend::from_env();
    let output = Command::new(ENGINE)
        .args(["--prompt", USER, "--system", SYSTEM, "--max-tokens", "48"])
        .arg("--model")
        .arg(&model)
        .args(backend.cli_args())
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["backend"], backend.name(), "{v}");
    assert_eq!(v["finish_reason"], "stop", "{v}");
    let text = v["text"].as_str().unwrap();
    assert!(text.to_lowercase().contains("tomorrow"), "{text}");
    assert!(!text.contains("<think>"), "{text}");
    assert!(v["tokens"].as_u64().unwrap() > 0);
    // Logs never carry the prompt or the answer.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("ship this thing"), "{stderr}");
    assert!(!stderr.contains(text), "{stderr}");
    // Without --json the text alone is printed.
    let plain = Command::new(ENGINE)
        .args(["--prompt", USER, "--system", SYSTEM, "--max-tokens", "48"])
        .arg("--model")
        .arg(&model)
        .args(backend.cli_args())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&plain.stdout).trim(), text);
}

/// `backend_preference = vulkan` is Vulkan or a refusal: a build without
/// the backend answers `load_failed` naming the preference instead of
/// opening the model on the CPU.
#[test]
fn a_vulkan_preference_on_a_cpu_build_is_refused_never_loaded_on_the_cpu() {
    if cfg!(feature = "vulkan") {
        eprintln!("skip: a Vulkan build answers by its device");
        return;
    }
    let Some(model) = skip() else { return };
    // Nothing forced (`DETTIVO_FORCE_CPU=1` would win over the
    // preference by design), so the preference alone decides.
    let mut e = Engine::spawn_on(Backend::Vulkan);
    let (reply, _) = e.call(Frame::request(
        1,
        "load",
        json!({"model": model.to_string_lossy(), "backend_preference": "vulkan"}),
    ));
    assert_eq!(reply.name, "error", "{reply:?}");
    assert_eq!(reply.payload["code"], "load_failed");
    let message = reply.payload["message"].as_str().unwrap();
    assert!(message.contains("backend_preference = vulkan"), "{message}");
    assert!(message.contains("no Vulkan backend"), "{message}");
    let (status, _) = e.call(Frame::request(2, "status", json!({})));
    assert_eq!(status.payload["loaded"], false, "{status:?}");
}

#[test]
fn a_corrupt_model_is_load_failed_naming_the_path_and_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.gguf");
    std::fs::write(&path, b"GGUF but not really").unwrap();
    let output = Command::new(ENGINE)
        .args(["--prompt", "hi", "--cpu"])
        .arg("--model")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("load_failed"), "{stderr}");
    assert!(stderr.contains("broken.gguf"), "{stderr}");
    assert!(stderr.contains("could not load"), "{stderr}");
}

struct Engine {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Engine {
    fn spawn() -> Self {
        Self::spawn_on(Backend::from_env())
    }

    /// The engine process pinned to `backend`: the CPU through
    /// `DETTIVO_FORCE_CPU=1`, Vulkan with nothing forced (the `load`
    /// carries the strict preference).
    fn spawn_on(backend: Backend) -> Self {
        let mut command = Command::new(ENGINE);
        if backend == Backend::Cpu {
            command.env("DETTIVO_FORCE_CPU", "1");
        } else {
            command.env_remove("DETTIVO_FORCE_CPU");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, frame: Frame) {
        write_frame(&mut self.stdin, &frame, &[]).unwrap();
        self.stdin.flush().unwrap();
    }

    fn next(&mut self) -> Frame {
        read_frame(&mut self.stdout).unwrap().0
    }

    /// Sends one request and collects everything up to its response.
    fn call(&mut self, frame: Frame) -> (Frame, Vec<Frame>) {
        let id = frame.id;
        self.send(frame);
        let mut events = Vec::new();
        loop {
            let f = self.next();
            if f.kind == Kind::Response && f.id == id {
                return (f, events);
            }
            events.push(f);
        }
    }
}

#[test]
fn the_protocol_streams_partials_and_stops_on_cancel_between_tokens() {
    let Some(model) = skip() else { return };
    let backend = Backend::from_env();
    let mut e = Engine::spawn();
    let (loaded, events) = e.call(Frame::request(
        1,
        "load",
        json!({"model": model.to_string_lossy(), "backend_preference": backend.name(), "context_length": 1024}),
    ));
    assert_eq!(loaded.name, "load", "{loaded:?}");
    assert_eq!(loaded.payload["backend"], backend.name(), "{loaded:?}");
    assert!(events.iter().any(|f| f.name == "loaded"));

    let (done, events) = e.call(Frame::request(
        2,
        "generate",
        json!({"prompt_profile": "polish", "system": SYSTEM, "user": USER, "max_tokens": 48, "temperature": 0.0}),
    ));
    assert_eq!(done.name, "generate", "{done:?}");
    assert_eq!(done.payload["finish_reason"], "stop");
    let partials: String = events
        .iter()
        .filter(|f| f.name == "partial")
        .map(|f| f.payload["text"].as_str().unwrap_or(""))
        .collect();
    assert!(!partials.is_empty(), "partials streamed");
    assert_eq!(partials, done.payload["text"].as_str().unwrap());

    // A long generation is cancelled after the first partial; the answer
    // so far comes back with `cancelled`, and the cancel itself is
    // acknowledged afterwards, in order.
    e.send(Frame::request(
        3,
        "generate",
        json!({"prompt_profile": "raw", "user": "Count from one to five hundred, one number per line:\n1\n2\n3\n", "max_tokens": 400, "temperature": 0.0}),
    ));
    let mut saw_busy = false;
    let mut sent_cancel = false;
    let mut partials = 0;
    let generate;
    loop {
        let f = e.next();
        match (f.kind, f.name.as_str(), f.id) {
            (Kind::Event, "partial", 3) => {
                partials += 1;
                if !sent_cancel {
                    e.send(Frame::request(4, "status", json!({})));
                    e.send(Frame::request(5, "cancel", json!({"request_id": 3})));
                    sent_cancel = true;
                }
            }
            (Kind::Response, "status", 4) => {
                assert_eq!(f.payload["busy"], true, "{f:?}");
                saw_busy = true;
            }
            (Kind::Response, "generate", 3) => {
                generate = f;
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(
        generate.payload["finish_reason"], "cancelled",
        "{generate:?}"
    );
    assert!(generate.payload["tokens"].as_u64().unwrap() < 400);
    assert!(
        partials >= 1 && saw_busy,
        "partials={partials} busy={saw_busy}"
    );
    let ack = e.next();
    assert_eq!(
        (ack.kind, ack.name.as_str(), ack.id),
        (Kind::Response, "cancel", 5)
    );

    let (status, _) = e.call(Frame::request(6, "status", json!({})));
    assert_eq!(status.payload["loaded"], true);
    assert_eq!(status.payload["busy"], false);
    let (unloaded, _) = e.call(Frame::request(7, "unload", json!({})));
    assert_eq!(unloaded.name, "unload");
    let (status, _) = e.call(Frame::request(8, "status", json!({})));
    assert_eq!(status.payload["loaded"], false);
    drop(e.stdin);
    assert!(e.child.wait().unwrap().success());
}
