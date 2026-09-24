//! `meeting_throughput` (ADR 0039): five minutes of two-source meeting
//! audio (the alpha microphone track once, then the jfk clip repeated
//! with silence between, mixed onto one clock) goes through
//! `transcripts.import { target_kind: "meeting" }` on a daemon in its
//! own profile, so the chunked pipeline, the merger and the store are all
//! timed. The realtime factor is the audio length over the wall time
//! from the job's first `decoding` progress to `done`; the engine, model,
//! backend and tier come from the daemon itself, the harness's own CPU
//! share is measured beside the figure, and the GPU sampler runs for the
//! whole job so `gpu_workload_proof` can read what it saw.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::RunOptions;
use super::gpu_proof;
use crate::nfr;
use crate::profile::{ModelSource, Profile};
use crate::scenarios::{self, daemon::DaemonHandle};

/// Audio the import transcribes.
pub const AUDIO_SECONDS: u64 = 300;
/// The harness share above which the figure carries a warning.
pub const HARNESS_CPU_WARN_PCT: f64 = 10.0;
/// How long the daemon may take to answer a ping.
const START_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the import may take before the step fails.
const JOB_TIMEOUT: Duration = Duration::from_secs(1800);

/// `meeting-throughput.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Throughput {
    /// `whisper`.
    pub engine: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported (`vulkan` or `cpu`).
    pub backend: String,
    /// The engine's own reason for that backend.
    pub reason: String,
    /// The daemon's `platform.tier`.
    pub tier: String,
    /// The daemon's `platform.tier_reason`.
    pub tier_reason: String,
    /// The audio imported.
    pub audio_ms: u64,
    /// From the first `decoding` progress to `done`.
    pub wall_ms: u64,
    /// From the import request to `done` (the upload excluded).
    pub job_wall_ms: u64,
    /// `audio_ms / wall_ms`.
    pub realtime_factor: f64,
    /// The NFR the tier is held to.
    pub nfr: String,
    /// The calibrated target.
    pub target: f64,
    /// Whether the factor reaches it.
    pub met: bool,
    /// The fixture's SHA-256.
    pub fixture_sha256: String,
    /// The harness's CPU share while the daemon was idle, before the job.
    pub harness_idle_cpu_pct: f64,
    /// The harness's CPU share during the job.
    pub harness_cpu_pct: f64,
    /// Set when the harness share passed the warning line.
    pub harness_warning: Option<String>,
    /// Segments on the row after the import.
    pub segments: usize,
    /// Words in the transcript.
    pub words: usize,
    /// Progress chunks seen.
    pub chunks_total: u64,
    /// `DETTIVO_FORCE_CPU=1` was set.
    pub forced_cpu: bool,
    /// The machine's 1 minute load at the start and the end.
    pub load_1m: (f64, f64),
}

/// The fixture the throughput steps share: the mixed meeting and the
/// system track alone.
pub struct Fixture {
    /// The mixed five-minute meeting.
    pub mixed: PathBuf,
    /// The system track alone (the jfk clip repeated).
    pub system: PathBuf,
    /// The mixed file's length.
    pub audio_ms: u64,
}

fn read_i16(path: &Path) -> Result<Vec<i16>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 || spec.bits_per_sample != 16 {
        return Err(format!("{} is not 16 kHz mono 16-bit", path.display()));
    }
    reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())
}

fn write_i16(path: &Path, samples: &[i16]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        writer.write_sample(*s).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

/// Builds the fixture under `dir` (or reads it back when both files are
/// there): the alpha microphone track, a second of silence, then the jfk
/// clip with a second of silence after each copy until `AUDIO_SECONDS`.
pub fn fixture(repo_root: &Path, models_dir: Option<&Path>, dir: &Path) -> Result<Fixture, String> {
    let mixed = dir.join("meeting.wav");
    let system = dir.join("system.wav");
    if mixed.is_file() && system.is_file() {
        let audio_ms = read_i16(&mixed)?.len() as u64 / 16;
        return Ok(Fixture {
            mixed,
            system,
            audio_ms,
        });
    }
    let clip = models_dir
        .map(|m| m.join("fixtures/jfk.wav"))
        .filter(|p| p.is_file())
        .ok_or(
            "jfk.wav is missing under the model directory (scripts/models/fetch-test-model.sh)",
        )?;
    let alpha = repo_root.join("crates/dettivo-qa/fixtures/meetings/alpha.mic.wav");
    let jfk = read_i16(&clip)?;
    let mic = read_i16(&alpha)?;
    let silence = vec![0i16; 16_000];
    let mut system_track = Vec::new();
    let mut lead = mic.clone();
    lead.extend_from_slice(&silence);
    while (lead.len() + system_track.len()) < (AUDIO_SECONDS * 16_000) as usize {
        system_track.extend_from_slice(&jfk);
        system_track.extend_from_slice(&silence);
    }
    let mut both = lead;
    both.extend_from_slice(&system_track);
    write_i16(&mixed, &both)?;
    write_i16(&system, &system_track)?;
    Ok(Fixture {
        mixed,
        system,
        audio_ms: both.len() as u64 / 16,
    })
}

/// The harness's own CPU time (user plus system) in clock ticks.
fn self_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let after = stat.rsplit(')').next().unwrap_or("");
    let fields: Vec<&str> = after.split_whitespace().collect();
    // Fields after the comm: state is 0, utime is index 11, stime 12.
    let parse = |i: usize| {
        fields
            .get(i)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    parse(11) + parse(12)
}

/// `/proc` accounts CPU time in clock ticks; Linux ships `CLK_TCK` at 100
/// on every distribution the product targets.
fn ticks_per_second() -> f64 {
    100.0
}

/// The harness's CPU share over `window`.
fn harness_share(window: Duration) -> f64 {
    let before = self_ticks();
    let at = Instant::now();
    std::thread::sleep(window);
    share_since(before, at)
}

fn share_since(ticks_before: u64, since: Instant) -> f64 {
    let ticks = self_ticks().saturating_sub(ticks_before) as f64 / ticks_per_second();
    let wall = since.elapsed().as_secs_f64().max(1e-6);
    ((ticks / wall) * 1000.0).round() / 10.0
}

fn load_1m() -> f64 {
    crate::bench::load_average().map(|l| l[0]).unwrap_or(0.0)
}

fn upload(daemon: &DaemonHandle, bytes: &[u8]) -> Result<String, String> {
    let begun = daemon.call(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "audio/wav", "size_hint": bytes.len()}),
    )?;
    let id = begun["transfer_id"]
        .as_str()
        .ok_or_else(|| format!("transfer.begin: {begun}"))?
        .to_string();
    let mut total = 0;
    for (i, chunk) in bytes.chunks(512 * 1024).enumerate() {
        daemon.call(
            "transfer.chunk",
            json!({"transfer_id": id, "seq": i + 1, "data_b64": base64::engine::general_purpose::STANDARD.encode(chunk)}),
        )?;
        total += 1;
    }
    daemon.call(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": total, "sha256": format!("{:x}", Sha256::digest(bytes))}),
    )?;
    Ok(id)
}

/// The daemon for the step: the profile's own tree with the models linked
/// in, the engines from `--engines` or beside `dettivod`, the model
/// pinned for meetings, diarization and analysis off so the transcription
/// alone is timed.
fn start_daemon(
    opts: &RunOptions,
    profile: &mut Profile,
    log: &Path,
    model: &str,
) -> Result<DaemonHandle, String> {
    let dettivod = scenarios::binary(&opts.repo_root, "dettivod")?;
    let engines = opts
        .meetings
        .engines_dir
        .clone()
        .or_else(|| dettivod.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let config = format!(
        "[engines]\ndirectory = \"{}\"\nstt_idle_seconds = 600\n[speech]\nprovider = \"whisper\"\nmodel = \"{model}\"\nmeeting_model = \"{model}\"\n[dictation]\nlanguage = \"en\"\n[meetings.diarization]\nauto = false\n[meetings.analysis]\nauto = false\n",
        engines.display()
    );
    let mut env: BTreeMap<String, String> = opts.meetings.scenario_env();
    env.insert("DETTIVO_MOCK_MODE".into(), "0".into());
    env.insert("DETTIVO_MOCK_INSERT".into(), "1".into());
    env.insert("DETTIVO_MOCK_A11Y".into(), "1".into());
    DaemonHandle::spawn_logged(&dettivod, profile, &config, &env, START_TIMEOUT, Some(log))
}

/// Runs the step into `dir`; the result, or why it failed.
pub fn run(opts: &RunOptions, dir: &Path) -> Result<Throughput, String> {
    let model = opts.meetings.model(opts.models_dir.as_deref());
    let model_file = opts.models_dir.as_ref().map(|m| {
        m.join("whisper")
            .join(&model)
            .join(format!("ggml-{model}.bin"))
    });
    if !model_file.as_ref().is_some_and(|p| p.is_file()) {
        return Err(format!(
            "whisper/{model} is not on disk; run `dettivo speech download --model {model}`"
        ));
    }
    let fixture = fixture(&opts.repo_root, opts.models_dir.as_deref(), dir)?;
    let bytes = std::fs::read(&fixture.mixed).map_err(|e| e.to_string())?;
    let fixture_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let source = opts
        .models_dir
        .clone()
        .map(|real| ModelSource::new(real, &opts.repo_root));
    let mut profile = Profile::create("meeting-throughput", source.as_ref())
        .map_err(|e| format!("profile: {e}"))?;
    profile.link_model(&format!("whisper/{model}"))?;
    let load_start = load_1m();
    drive(
        opts,
        &mut profile,
        dir,
        &model,
        &fixture,
        bytes,
        fixture_sha256,
        load_start,
    )
}

#[allow(clippy::too_many_arguments)]
fn drive(
    opts: &RunOptions,
    profile: &mut Profile,
    dir: &Path,
    model: &str,
    fixture: &Fixture,
    bytes: Vec<u8>,
    fixture_sha256: String,
    load_start: f64,
) -> Result<Throughput, String> {
    let mut daemon = start_daemon(opts, profile, &dir.join("daemon.log"), model)?;
    let daemon_pid = daemon.pid().ok_or("the daemon has no pid")?;
    // The harness at rest, with the daemon up and nothing in flight.
    let harness_idle_cpu_pct = harness_share(Duration::from_secs(1));
    let mut events = daemon.subscribe(&["job.progress"])?;
    // A CPU chunk can take longer than the stream's ten-second read wait
    // between two progress events; the job's own deadline is the limit.
    events.set_read_timeout(JOB_TIMEOUT)?;
    let transfer = upload(&daemon, &bytes)?;
    let sampler = gpu_proof::Sampler::start(daemon_pid, dir.join("gpu-samples.json"));
    let ticks_before = self_ticks();
    let job_started = Instant::now();
    let started = daemon.call(
        "transcripts.import",
        json!({
            "transfer_id": transfer, "target_kind": "meeting", "filename": "meeting.wav",
            "language": "en", "mode": "raw", "acknowledge_meeting_disclosure": true,
            "provider": "whisper", "model": model, "diarize": false, "analyze": false
        }),
    )?;
    let job_id = started["job"]["job_id"]
        .as_str()
        .ok_or_else(|| format!("transcripts.import: {started}"))?
        .to_string();
    let meeting_id = started["ref"]["id"]
        .as_str()
        .ok_or_else(|| format!("transcripts.import: {started}"))?
        .to_string();
    let mut decoding_at: Option<Instant> = None;
    let mut done_at: Option<Instant> = None;
    let mut chunks_total = 0;
    let mut failure: Option<String> = None;
    let seen = events.collect(
        |p| {
            if p["payload"]["job_id"] != json!(job_id) {
                return false;
            }
            let stage = p["payload"]["stage"].as_str().unwrap_or("");
            if decoding_at.is_none() && stage == "decoding" {
                decoding_at = Some(Instant::now());
            }
            if let Some(n) = p["payload"]["chunks_total"].as_u64() {
                chunks_total = chunks_total.max(n);
            }
            match stage {
                "done" => {
                    done_at = Some(Instant::now());
                    true
                }
                "failed" | "cancelled" => {
                    failure = Some(format!("the import {stage}: {}", p["payload"]));
                    true
                }
                _ => false,
            }
        },
        JOB_TIMEOUT,
    );
    let harness_cpu_pct = share_since(ticks_before, job_started);
    let samples = sampler.stop();
    let lines: Vec<String> = seen.iter().map(Value::to_string).collect();
    let _ = std::fs::write(dir.join("progress.jsonl"), lines.join("\n") + "\n");
    if let Some(why) = failure {
        return Err(why);
    }
    let Some(done_at) = done_at else {
        return Err(format!(
            "the import did not finish within {JOB_TIMEOUT:?} ({} progress events)",
            seen.len()
        ));
    };
    let decoding_at = decoding_at.unwrap_or(job_started);
    let wall_ms =
        u64::try_from(done_at.duration_since(decoding_at).as_millis()).unwrap_or(u64::MAX);
    let job_wall_ms =
        u64::try_from(done_at.duration_since(job_started).as_millis()).unwrap_or(u64::MAX);
    let got = daemon.call("meetings.get", json!({"meeting_id": meeting_id}))?;
    let engines = daemon.call("speech.engines", json!({}))?;
    let caps = daemon.call("system.capabilities", json!({}))?;
    daemon.stop();
    let _ = std::fs::write(
        dir.join("transcript.json"),
        serde_json::to_string_pretty(&got["segments"]).unwrap_or_default(),
    );
    let engine = engines["engines"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["binary"] == json!("dettivo-engine-whisper"))
        .cloned()
        .unwrap_or(Value::Null);
    let backend = engine["backend"].as_str().unwrap_or("unknown").to_string();
    if opts.meetings.cpu && backend == "vulkan" {
        return Err(
            "dettivo-engine-whisper reports the vulkan backend under --cpu (DETTIVO_FORCE_CPU=1 was set)".into(),
        );
    }
    let platform = &caps["platform"];
    let tier_name = platform["tier"].as_str().unwrap_or("cpu").to_string();
    let tier = if tier_name == "gpu" {
        nfr::Tier::Gpu
    } else {
        nfr::Tier::Cpu
    };
    let target = nfr::stt_realtime(tier);
    let factor = fixture.audio_ms as f64 / wall_ms.max(1) as f64;
    let realtime_factor = (factor * 100.0).round() / 100.0;
    let transcript = got["transcript"].as_str().unwrap_or("");
    let words = crate::pack::wer::words(transcript).len();
    if words == 0 {
        return Err("the import completed with an empty transcript".into());
    }
    let harness_warning = (harness_cpu_pct > HARNESS_CPU_WARN_PCT).then(|| {
        format!(
            "the harness used {harness_cpu_pct:.1}% of a core during the job (over {HARNESS_CPU_WARN_PCT}%): the load was not the product's alone"
        )
    });
    let result = Throughput {
        engine: "whisper".into(),
        model: model.to_string(),
        backend,
        reason: engine["reason"].as_str().unwrap_or("").to_string(),
        tier: tier_name,
        tier_reason: platform["tier_reason"].as_str().unwrap_or("").to_string(),
        audio_ms: fixture.audio_ms,
        wall_ms,
        job_wall_ms,
        realtime_factor,
        nfr: target.nfr.to_string(),
        target: target.calibrated,
        met: target.met(factor),
        fixture_sha256,
        harness_idle_cpu_pct,
        harness_cpu_pct,
        harness_warning,
        segments: got["segments"].as_array().map(Vec::len).unwrap_or(0),
        words,
        chunks_total,
        forced_cpu: opts.meetings.cpu,
        load_1m: (load_start, load_1m()),
    };
    let _ = samples;
    std::fs::write(
        dir.join("meeting-throughput.json"),
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_leads_with_the_alpha_track_and_repeats_the_clip() {
        let dir = tempfile::tempdir().unwrap();
        let models = dir.path().join("models/fixtures");
        std::fs::create_dir_all(&models).unwrap();
        // A two-second stand-in for the clip.
        write_i16(&models.join("jfk.wav"), &vec![1000i16; 32_000]).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let f = fixture(&root, Some(&dir.path().join("models")), &out).unwrap();
        assert!(f.audio_ms >= AUDIO_SECONDS * 1000, "{}", f.audio_ms);
        let mixed = read_i16(&f.mixed).unwrap();
        let system = read_i16(&f.system).unwrap();
        assert!(mixed.len() > system.len());
        // The system track starts with the clip, the mixed file with the
        // alpha lead (silence) then Alice.
        assert_eq!(system[0], 1000);
        assert_eq!(mixed[0], 0);
        let again = fixture(&root, None, &out).unwrap();
        assert_eq!(again.audio_ms, f.audio_ms);
    }

    #[test]
    fn the_harness_share_reads_its_own_clock() {
        assert!(ticks_per_second() >= 1.0);
        let share = harness_share(Duration::from_millis(20));
        assert!((0.0..=100.0 * 64.0).contains(&share), "{share}");
    }
}
