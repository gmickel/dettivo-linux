//! A speech engine process driven over the engine protocol, for the steps
//! that time what the daemon cannot separate: the load, the first
//! inference after it, the warm-up and the inferences after that. One
//! request at a time; the engine's stderr lands in a log beside the
//! report.

use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_engine_proto::{Frame, Kind, pcm_to_bytes, read_frame, write_frame};
use serde_json::{Value, json};

/// The warm-up's silence, as the supervisor sends it: 30 s at 16 kHz.
pub const WARM_UP_SAMPLES: usize = 480_000;

/// The longest one request may take, its frames written and its answer
/// read: a large model's load or the warm-up on the CPU finishes well
/// inside it, and an engine still silent then is killed so its step fails.
const CALL_TIMEOUT: Duration = Duration::from_secs(300);

/// A running engine process.
pub struct EngineClient {
    child: Arc<Mutex<Child>>,
    writer: BufWriter<ChildStdin>,
    reader: BufReader<ChildStdout>,
    next_id: u64,
    limit: Duration,
}

impl EngineClient {
    /// Spawns `binary` with its stderr appended to `log`.
    pub fn spawn(binary: &Path, force_cpu: bool, log: &Path) -> Result<Self, String> {
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| format!("{}: {e}", log.display()))?;
        let mut cmd = Command::new(binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr));
        if force_cpu {
            cmd.env("DETTIVO_FORCE_CPU", "1");
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{}: {e}", binary.display()))?;
        let writer = BufWriter::new(child.stdin.take().ok_or("engine stdin")?);
        let reader = BufReader::new(child.stdout.take().ok_or("engine stdout")?);
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            writer,
            reader,
            next_id: 1,
            limit: CALL_TIMEOUT,
        })
    }

    /// The process id.
    pub fn pid(&self) -> u32 {
        self.child.lock().unwrap_or_else(|p| p.into_inner()).id()
    }

    /// Sends `name` and waits for its response, skipping events; an
    /// error response is an `Err` with the engine's message. An engine
    /// that has not answered within `limit` is killed, which also ends a
    /// read or write it left blocked mid-frame, and the call fails.
    pub fn call(
        &mut self,
        name: &str,
        payload: Value,
        pcm: Option<&[i16]>,
    ) -> Result<Value, String> {
        let (done, finished) = mpsc::channel::<()>();
        let (child, limit) = (self.child.clone(), self.limit);
        let watchdog = std::thread::spawn(move || {
            let expired = finished.recv_timeout(limit) == Err(RecvTimeoutError::Timeout);
            if expired {
                let _ = child.lock().unwrap_or_else(|p| p.into_inner()).kill();
            }
            expired
        });
        let result = self.exchange(name, payload, pcm);
        drop(done);
        if watchdog.join().unwrap_or(true) {
            return Err(format!("{name}: no answer within {limit:?}; engine killed"));
        }
        result
    }

    fn exchange(
        &mut self,
        name: &str,
        payload: Value,
        pcm: Option<&[i16]>,
    ) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let bytes = pcm.map(pcm_to_bytes);
        let mut frame = Frame::request(id, name, payload);
        if let Some(b) = &bytes {
            frame = frame.with_pcm(b.len() as u64);
        }
        let attachments: Vec<&[u8]> = bytes.iter().map(Vec::as_slice).collect();
        write_frame(&mut self.writer, &frame, &attachments).map_err(|e| format!("{name}: {e}"))?;
        loop {
            let (reply, _) = read_frame(&mut self.reader).map_err(|e| format!("{name}: {e:?}"))?;
            if reply.kind != Kind::Response || reply.id != id {
                continue;
            }
            if reply.name == "error" {
                return Err(format!("{name}: {}", reply.payload));
            }
            return Ok(reply.payload);
        }
    }

    /// `load` with `threads` when given; the answer and its wall time.
    pub fn load(
        &mut self,
        model: &Path,
        threads: Option<u32>,
    ) -> Result<(Value, Duration), String> {
        let at = Instant::now();
        let loaded = self.call(
            "load",
            json!({
                "model": model.to_string_lossy(),
                "backend_preference": "auto",
                "threads": threads,
            }),
            None,
        )?;
        Ok((loaded, at.elapsed()))
    }

    /// One `recognize` of `pcm` in `language`; the answer and its wall time.
    pub fn recognize(&mut self, pcm: &[i16], language: &str) -> Result<(Value, Duration), String> {
        let at = Instant::now();
        let result = self.call(
            "recognize",
            json!({ "language": language, "prompt": null, "timestamps": false }),
            Some(pcm),
        )?;
        Ok((result, at.elapsed()))
    }

    /// The warm-up the supervisor runs after a load: 30 s of silence.
    pub fn warm_up(&mut self) -> Result<Duration, String> {
        self.recognize(&[0; WARM_UP_SAMPLES], "en")
            .map(|(_, took)| took)
    }
}

impl Drop for EngineClient {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap_or_else(|p| p.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Milliseconds of a duration, saturating.
pub fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// 16 kHz mono PCM from a WAV file.
pub fn read_pcm(path: &Path) -> Result<Vec<i16>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 || spec.bits_per_sample != 16 {
        return Err(format!(
            "{}: {} Hz, {} channels, {} bits; the engines take 16 kHz mono 16-bit",
            path.display(),
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample
        ));
    }
    reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// MiB of GPU memory in use on the first NVIDIA device, from
/// `nvidia-smi`; `None` without it (an AMD or Intel GPU reports nothing).
pub fn gpu_used_mib() -> Option<u64> {
    let out = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that never answers: `cat` echoes the request back and
    /// then waits, so a `load` blocks on the read, and the warm-up's 960 kB
    /// frame blocks on the write once it outgrows the pipe.
    #[test]
    fn an_engine_that_stops_answering_fails_the_call_instead_of_hanging() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let log = dir.path().join("engine.log");
        std::thread::spawn(move || {
            let silent = || {
                let mut engine = EngineClient::spawn(Path::new("cat"), false, &log).unwrap();
                engine.limit = Duration::from_millis(500);
                engine
            };
            let load = silent()
                .load(Path::new("/models/none.bin"), None)
                .map(|_| ());
            let warm_up = silent().warm_up().map(|_| ());
            let _ = tx.send([load, warm_up]);
        });
        let results = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the calls returned rather than hanging");
        for result in results {
            let error = result.expect_err("a silent engine fails the call");
            assert!(error.contains("no answer within"), "{error}");
        }
    }
}
