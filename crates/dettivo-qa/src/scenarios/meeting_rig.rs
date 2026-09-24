//! Two-source capture on the virtual audio rig (fn-24 R1 and R2): two null
//! sinks stand in for the microphone and the system output, the daemon is
//! pinned to them (`[audio] input_device`, `[audio] system_source`), a
//! meeting starts, one fixture plays into each sink, the microphone sink
//! is unloaded mid-meeting, and the takes are judged: each track
//! correlates with its own fixture and not the other's, the two first
//! takes start within 200 ms of each other on the meeting clock, the
//! microphone restarted as `microphone-2.wav` with a gap marker in the
//! journal (or, with no other source on the machine, the journal records
//! the gap and the system track carried on), the finalisation after the
//! stop runs over every take and completes the meeting (fn-28 R3), and
//! `metadata.json` lists the takes. Skipped without PipeWire; needs the
//! test model for the finalisation.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;

use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::audio;
use crate::driver::Driver;

/// The scenario.
pub struct MeetingRig;

/// What `meeting-rig.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    meeting_id: String,
    microphone_correlation: f64,
    system_correlation: f64,
    cross_correlation: f64,
    first_take_offset_gap_ms: u64,
    microphone_takes: usize,
    second_take_file: Option<String>,
    gap_recorded: bool,
    gap_detail: Option<String>,
    /// The sample of the system track at which the microphone sink went
    /// away, on the capture clock (16 kHz from the start answer).
    switch_boundary_sample: usize,
    system_samples_after_switch: u32,
    /// The second system fixture found after the boundary.
    post_switch_correlation: f64,
    metadata_takes: usize,
    settled_status: String,
    chunks_total: u64,
}

/// A one-second fixture at `hz` plus a partial, so the two tracks are
/// distinguishable by correlation.
fn write_tone(path: &Path, hz: f64) -> std::io::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(std::io::Error::other)?;
    for n in 0..16_000u32 {
        let t = f64::from(n) / 16_000.0;
        let v = 0.4 * (2.0 * std::f64::consts::PI * hz * t).sin()
            + 0.2 * (2.0 * std::f64::consts::PI * hz * 2.5 * t).sin();
        writer
            .write_sample((v * 32_767.0) as i16)
            .map_err(std::io::Error::other)?;
    }
    writer.finalize().map_err(std::io::Error::other)
}

fn read_mono(path: &Path) -> Result<Vec<f64>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(reader
        .samples::<i16>()
        .filter_map(Result::ok)
        .map(|s| f64::from(s) / 32_768.0)
        .collect())
}

fn play(sink: &str, file: &Path) -> Result<ChildOwner, String> {
    Command::new("pw-play")
        .args(["--target", sink])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(ChildOwner::new)
        .map_err(|e| format!("pw-play: {e}"))
}

fn rig(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(audio::rig_script(repo))
        .args(args)
        .output()
        .map_err(|e| format!("audio-rig: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "audio-rig {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

impl Scenario for MeetingRig {
    fn id(&self) -> &'static str {
        "meeting_rig"
    }

    fn summary(&self) -> &'static str {
        "two null sinks feed a meeting's microphone and system tracks; the microphone sink is unloaded mid-meeting"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preflight(&self) -> Result<(), String> {
        if !audio::pipewire_available() {
            return Err("PipeWire is not running (pactl info failed)".into());
        }
        for tool in ["pw-play", "pactl"] {
            if Command::new(tool).arg("--version").output().is_err() {
                return Err(format!("{tool} is not installed"));
            }
        }
        Ok(())
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let pid = std::process::id();
        let mic_sink = format!("dettivo-qa-mic-{pid}");
        let sys_sink = format!("dettivo-qa-sys-{pid}");
        rig(ctx.repo_root, &["up", &mic_sink])?;
        let result = self.drive(ctx, &dettivod, &mic_sink, &sys_sink);
        let _ = rig(ctx.repo_root, &["down", &mic_sink]);
        let _ = rig(ctx.repo_root, &["down", &sys_sink]);
        result
    }
}

impl MeetingRig {
    fn drive(
        &self,
        ctx: &mut Context<'_>,
        dettivod: &Path,
        mic_sink: &str,
        sys_sink: &str,
    ) -> Result<(), String> {
        rig(ctx.repo_root, &["up", sys_sink])?;
        let mic_fixture = ctx.evidence_dir.join("microphone-fixture.wav");
        let sys_fixture = ctx.evidence_dir.join("system-fixture.wav");
        write_tone(&mic_fixture, 440.0).map_err(|e| e.to_string())?;
        write_tone(&sys_fixture, 1300.0).map_err(|e| e.to_string())?;
        // The tones are not speech: the live path stays off and the
        // finalisation after the stop proves every take goes through.
        let bin_dir = dettivod.parent().map(Path::to_path_buf).unwrap_or_default();
        let config = format!(
            "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n[audio]\ninput_device = \"{mic_sink}\"\nsystem_source = \"{sys_sink}\"\n[meetings]\nlive = false\n",
            bin_dir.display()
        );
        // The rig's own daemon captures PipeWire for real: no mock switches.
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        env.insert("DETTIVO_MOCK_MODE".into(), "0".into());
        env.insert("DETTIVO_MOCK_INSERT".into(), "1".into());
        let mut daemon = DaemonHandle::spawn(dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        let devices = daemon.call("audio.devices", json!({}))?;
        let listed = devices["devices"]
            .as_array()
            .map(|d| {
                d.iter()
                    .filter(|n| n["name"] == json!(mic_sink) || n["name"] == json!(sys_sink))
                    .count()
            })
            .unwrap_or(0);
        if listed != 2 {
            return Err(format!(
                "audio.devices does not list both rig sinks: {devices}"
            ));
        }
        ctx.timings.mark("daemon");

        let started = daemon.call(
            "meetings.start",
            json!({"capture": {"microphone": true, "system_audio": true}, "title": "Rig", "acknowledge_meeting_disclosure": true}),
        )?;
        let id = started["ref"]["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("no ref.id: {started}"))?;
        // The capture clock starts with the answer; the capture path's
        // own latency only moves the boundary later, into the gap before
        // the second fixture, never earlier into the first.
        let capture_started = std::time::Instant::now();
        std::thread::sleep(Duration::from_millis(400));
        let mut mic_player = play(mic_sink, &mic_fixture)?;
        let mut sys_player = play(sys_sink, &sys_fixture)?;
        let _ = mic_player.wait();
        let _ = sys_player.wait();
        std::thread::sleep(Duration::from_millis(300));
        ctx.timings.mark("played");

        // The device switch: the microphone sink goes away mid-meeting.
        let switch_boundary_sample =
            (capture_started.elapsed().as_secs_f64() * CAPTURE_HZ) as usize;
        rig(ctx.repo_root, &["down", mic_sink])?;
        std::thread::sleep(Duration::from_millis(1200));
        let mut sys_player = play(sys_sink, &sys_fixture)?;
        let _ = sys_player.wait();
        std::thread::sleep(Duration::from_millis(300));
        ctx.timings.mark("switched");

        daemon.call("meetings.stop", json!({"meeting_id": id}))?;
        // The stop closes the takes, then the finalisation runs over every
        // one of them (both microphone takes and the system track).
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        let settled = loop {
            let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
            if matches!(
                status["status"].as_str(),
                Some("completed" | "failed" | "stopped")
            ) && status["is_finalizing"] != json!(true)
            {
                break status;
            }
            if std::time::Instant::now() > deadline {
                return Err(format!("the meeting never settled: {status}"));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        daemon.stop();
        ctx.timings.mark("stopped");

        let dir = ctx.profile.root.join("data/dettivo/meetings").join(&id);
        let mic = read_mono(&dir.join("microphone.wav"))?;
        let sys = read_mono(&dir.join("system.wav"))?;
        let mic_fx = read_mono(&mic_fixture)?;
        let sys_fx = read_mono(&sys_fixture)?;
        // The system fixture plays twice (once more after the switch), so
        // its correlation is judged over the first two and a half seconds,
        // where exactly one copy sits; the microphone fixture played once.
        let head = &sys[..sys.len().min(40_000)];
        let microphone_correlation = audio::correlation(&mic_fx, &mic, 48_000);
        let system_correlation = audio::correlation(&sys_fx, head, 40_000);
        let cross_correlation = audio::correlation(&sys_fx, &mic, 48_000);
        let mic_takes = dettivo_audio_sidecar(&dir.join("takes.json"))?;
        let sys_takes = dettivo_audio_sidecar(&dir.join("system-takes.json"))?;
        let first_gap = mic_takes[0]["start_offset_ms"]
            .as_u64()
            .unwrap_or(0)
            .abs_diff(sys_takes[0]["start_offset_ms"].as_u64().unwrap_or(0));
        let journal = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap_or_default();
        let gap_line = journal
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|e| e["event"] == json!("gap"));
        let metadata: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("metadata.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let metadata_takes = metadata["takes"]
            .as_array()
            .map(|t| {
                t.iter()
                    .map(|x| x["takes"].as_array().map(Vec::len).unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0);
        let (system_samples_after_switch, post_switch_correlation) =
            after_switch(&sys, &sys_fx, switch_boundary_sample);

        let evidence = Evidence {
            meeting_id: id,
            microphone_correlation,
            system_correlation,
            cross_correlation,
            first_take_offset_gap_ms: first_gap,
            microphone_takes: mic_takes.len(),
            second_take_file: mic_takes
                .get(1)
                .and_then(|t| t["file"].as_str().map(str::to_string)),
            gap_recorded: gap_line.is_some(),
            gap_detail: gap_line
                .as_ref()
                .and_then(|g| g["detail"].as_str().map(str::to_string)),
            switch_boundary_sample,
            system_samples_after_switch,
            post_switch_correlation,
            metadata_takes,
            settled_status: settled["status"].as_str().unwrap_or("").to_string(),
            chunks_total: settled["capture"]["chunks_total"].as_u64().unwrap_or(0),
        };
        std::fs::write(
            ctx.evidence_dir.join("meeting-rig.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("meeting-rig.json".into());
        for name in [
            "microphone.wav",
            "system.wav",
            "journal.jsonl",
            "metadata.json",
        ] {
            if std::fs::copy(dir.join(name), ctx.evidence_dir.join(name)).is_ok() {
                ctx.evidence.push(name.into());
            }
        }

        if microphone_correlation < 0.8 || system_correlation < 0.8 {
            return Err(format!(
                "the takes do not match their fixtures: microphone {microphone_correlation:.3}, system {system_correlation:.3}"
            ));
        }
        if cross_correlation > 0.5 {
            return Err(format!(
                "the microphone take also matches the system fixture ({cross_correlation:.3}): the tracks are not separate"
            ));
        }
        if first_gap > 200 {
            return Err(format!(
                "the first takes start {first_gap} ms apart on the meeting clock"
            ));
        }
        if evidence.gap_detail.is_none() {
            return Err(
                "the journal has no gap marker after the microphone sink was unloaded".into(),
            );
        }
        if mic_takes.len() >= 2 && evidence.second_take_file.as_deref() != Some("microphone-2.wav")
        {
            return Err(format!(
                "the second take is not microphone-2.wav: {:?}",
                evidence.second_take_file
            ));
        }
        if system_samples_after_switch < 8_000 || post_switch_correlation < 0.8 {
            return Err(format!(
                "the system track did not carry on after the switch ({system_samples_after_switch} samples past the switch at sample {switch_boundary_sample}, fixture match {post_switch_correlation:.3})"
            ));
        }
        if metadata_takes != mic_takes.len() + sys_takes.len() {
            return Err(format!(
                "metadata.json lists {metadata_takes} takes, the sidecars {}",
                mic_takes.len() + sys_takes.len()
            ));
        }
        if evidence.settled_status != "completed" {
            return Err(format!(
                "the finalisation did not complete the meeting: {}",
                evidence.settled_status
            ));
        }
        if evidence.chunks_total < (mic_takes.len() + sys_takes.len()) as u64 {
            return Err(format!(
                "the finalisation covered {} chunks for {} takes",
                evidence.chunks_total,
                mic_takes.len() + sys_takes.len()
            ));
        }
        Ok(())
    }
}

/// The capture-clock sample rate of the takes (the fixtures' rate).
const CAPTURE_HZ: f64 = 16_000.0;

/// The system track after the device switch: the samples past
/// `boundary` (the switch instant on the capture clock) and how well the
/// fixture that played after the switch is found in them. A track that
/// stops at the switch has few samples and no match; audio recorded
/// before the switch never counts.
fn after_switch(system: &[f64], fixture: &[f64], boundary: usize) -> (u32, f64) {
    let after = &system[boundary.min(system.len())..];
    let correlation = audio::correlation(fixture, after, after.len());
    (after.len() as u32, correlation)
}

/// The takes of a sidecar.
fn dettivo_audio_sidecar(path: &Path) -> Result<Vec<serde_json::Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    doc["takes"]
        .as_array()
        .cloned()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| format!("{} lists no takes", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, seconds: f64) -> Vec<f64> {
        (0..(CAPTURE_HZ * seconds) as usize)
            .map(|i| (i as f64 * hz * std::f64::consts::TAU / CAPTURE_HZ).sin() * 0.5)
            .collect()
    }

    #[test]
    fn only_audio_after_the_switch_boundary_proves_the_system_track_carried_on() {
        let fixture = tone(1300.0, 1.0);
        let boundary = (CAPTURE_HZ * 2.0) as usize;
        // The fixture before the switch, then the fixture again after it.
        let mut carried = Vec::new();
        carried.extend(tone(0.0, 0.4));
        carried.extend(&fixture);
        carried.extend(tone(0.0, 1.8));
        carried.extend(&fixture);
        carried.extend(tone(0.0, 0.3));
        let (samples, correlation) = after_switch(&carried, &fixture, boundary);
        assert!(samples >= 8_000, "{samples}");
        assert!(correlation >= 0.8, "{correlation}");

        // The same track cut at the switch: the first fixture alone used
        // to exceed the old one-second subtraction and pass.
        let truncated: Vec<f64> = carried[..boundary + 16_000].to_vec();
        let (samples, correlation) = after_switch(&truncated, &fixture, boundary);
        assert!(samples >= 8_000, "the old count check would pass this");
        assert!(correlation < 0.8, "{correlation}");
        let (samples, _) = after_switch(&carried[..boundary], &fixture, boundary);
        assert_eq!(samples, 0);
    }
}
