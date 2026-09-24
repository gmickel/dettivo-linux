//! The meeting helpers the `meetings*` test binaries share: the contract
//! fixtures, the mock track fixture, the QA environment with both mock
//! tracks, a tree with tiny.en selected, and a subscriber that collects
//! notifications until one satisfies a condition.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Daemon, Tree, local_model, tree_with_tiny};

pub const SAMPLE: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

pub fn fixture(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-proto/fixtures")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// A short 16 kHz tone the mock tracks play (the speech fixture when the
/// model directory has it, else a generated one).
pub fn track_fixture(name: &str) -> PathBuf {
    if let Some((_, wav)) = local_model() {
        return wav;
    }
    let path = std::env::temp_dir().join(format!("dtv-meeting-{name}-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for n in 0..160_000u32 {
        let v = (0.3
            * (2.0 * std::f64::consts::PI * 440.0 * f64::from(n) / 16_000.0).sin()
            * 32_767.0) as i16;
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
    path
}

pub fn env(
    mic: &Path,
    system: &Path,
    extra: &[(&'static str, &str)],
) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("DETTIVO_QA_MODE", "1".to_string()),
        ("DETTIVO_E2E_SEED", "1".to_string()),
        ("DETTIVO_MOCK_INSERT", "1".to_string()),
        ("DETTIVO_MOCK_MIC", mic.to_string_lossy().into_owned()),
        (
            "DETTIVO_MOCK_SYSTEM_AUDIO",
            system.to_string_lossy().into_owned(),
        ),
    ];
    out.extend(extra.iter().map(|(k, v)| (*k, (*v).to_string())));
    out
}

pub fn spawn(tree: Tree, env: &[(&'static str, String)]) -> Daemon {
    let borrowed: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    Daemon::spawn(tree, &borrowed)
}

/// A tree with tiny.en selected and `extra` appended to the configuration;
/// `None` (a skip) without the local model.
pub fn tree(extra: &str) -> Option<Tree> {
    let Some((model, _)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return None;
    };
    Some(tree_with_tiny(&model, extra))
}

pub fn start_params(title: &str) -> Value {
    json!({
        "capture": {"microphone": true, "system_audio": true},
        "title": title,
        "acknowledge_meeting_disclosure": true
    })
}

pub fn wait_status(daemon: &Daemon, id: &str, wanted: &str) -> Value {
    // Finalisation transcribes every take plus the system track through
    // tiny.en on the CPU; the two-core CI runner needs well over ten
    // seconds for the eighteen-second fixture meeting.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let status = daemon.result("meetings.status", json!({"meeting_id": id}));
        if status["status"] == wanted {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "{id} never reached {wanted}: {status}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub struct Subscriber {
    reader: BufReader<UnixStream>,
}

impl Subscriber {
    pub fn open(daemon: &Daemon, topics: &[&str]) -> Self {
        let mut stream = UnixStream::connect(daemon.tree.socket()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let req = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": topics, "buffer": 256}});
        stream.write_all(format!("{req}\n").as_bytes()).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        Self { reader }
    }

    pub fn collect(
        &mut self,
        mut stop: impl FnMut(&Value) -> bool,
        timeout: Duration,
    ) -> Vec<Value> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while Instant::now() < deadline {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let v: Value = serde_json::from_str(line.trim_end()).unwrap();
                    let done = stop(&v["params"]);
                    out.push(v["params"].clone());
                    if done {
                        break;
                    }
                }
            }
        }
        out
    }
}
