//! The virtual audio rig (R4): a PipeWire null sink fed by `pw-play`, its
//! monitor read back by `pw-record`, the recording compared with the
//! fixture, and an unload mid-recording detected by the recorder. The
//! shell side lives in `scripts/qa/audio-rig.sh`; this module drives it
//! and judges the round trip.

use crate::child::ChildOwner;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The outcome of the audio check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioReport {
    /// `pass`, `fail` or `skipped`.
    pub outcome: String,
    /// The sink that was created.
    pub sink: String,
    /// The monitor source the rig printed.
    pub monitor: Option<String>,
    /// Normalised cross-correlation between fixture and recording (0..1).
    pub correlation: Option<f64>,
    /// The tolerance the correlation had to reach.
    pub tolerance: f64,
    /// True when unloading the sink ended the recorder's stream.
    pub unload_detected: Option<bool>,
    /// Why, when not a pass.
    pub reason: Option<String>,
}

/// Writes a one-second 440 Hz plus 1 kHz fixture (16 kHz mono, 16-bit).
pub fn write_fixture(path: &Path) -> std::io::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(std::io::Error::other)?;
    for n in 0..16_000u32 {
        let t = f64::from(n) / 16_000.0;
        let v = 0.4 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
            + 0.3 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
        writer
            .write_sample((v * 32_767.0) as i16)
            .map_err(std::io::Error::other)?;
    }
    writer.finalize().map_err(std::io::Error::other)
}

fn read_mono(path: &Path) -> std::io::Result<(u32, Vec<f64>)> {
    let mut reader = hound::WavReader::open(path).map_err(std::io::Error::other)?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    let samples: Vec<f64> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .filter_map(Result::ok)
            .map(|s| f64::from(s) / ((1i64 << (spec.bits_per_sample - 1)) as f64))
            .collect(),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .filter_map(Result::ok)
            .map(f64::from)
            .collect(),
    };
    let mono: Vec<f64> = samples
        .chunks(channels)
        .map(|c| c.iter().sum::<f64>() / channels as f64)
        .collect();
    Ok((spec.sample_rate, mono))
}

/// Peak normalised cross-correlation of `b` against `a` over a lag search,
/// tolerant of the delay a real capture path adds.
pub fn correlation(a: &[f64], b: &[f64], max_lag: usize) -> f64 {
    let norm = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>()).sqrt().max(1e-9);
    let (na, nb) = (norm(a), norm(b));
    let mut best = 0.0f64;
    for lag in 0..=max_lag {
        let n = a.len().min(b.len().saturating_sub(lag));
        if n == 0 {
            break;
        }
        let dot: f64 = (0..n).map(|i| a[i] * b[i + lag]).sum();
        best = best.max(dot / (na * nb));
    }
    best
}

/// The rig script beside the repository.
pub fn rig_script(repo_root: &Path) -> PathBuf {
    repo_root.join("scripts/qa/audio-rig.sh")
}

fn rig(repo_root: &Path, args: &[&str]) -> std::io::Result<String> {
    let output = Command::new(rig_script(repo_root)).args(args).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "audio-rig {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// True when a PipeWire daemon answers on this session.
pub fn pipewire_available() -> bool {
    Command::new("pactl")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Runs the whole round trip: sink up, fixture played, monitor recorded,
/// correlation judged, then the sink unloaded under a second recorder.
pub fn check(repo_root: &Path, work: &Path, tolerance: f64) -> std::io::Result<AudioReport> {
    let sink = format!("dettivo-qa-{}", std::process::id());
    if !pipewire_available() {
        return Ok(AudioReport {
            outcome: "skipped".into(),
            sink,
            monitor: None,
            correlation: None,
            tolerance,
            unload_detected: None,
            reason: Some("PipeWire is not running (pactl info failed); the fixture microphone variable carries capture in this environment".into()),
        });
    }
    let fixture = work.join("fixture.wav");
    let recording = work.join("recording.wav");
    write_fixture(&fixture)?;
    let monitor = rig(repo_root, &["up", &sink])?;
    let result = (|| -> std::io::Result<AudioReport> {
        // Record 2.5 s from the monitor while the 1 s fixture plays into the sink.
        let mut recorder = Command::new("pw-record")
            .args([
                "--target",
                &sink,
                "-P",
                "{ stream.capture.sink = true }",
                "--rate",
                "16000",
                "--channels",
                "1",
                "--format",
                "s16",
            ])
            .arg(&recording)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(ChildOwner::new)?;
        std::thread::sleep(Duration::from_millis(400));
        rig(repo_root, &["play", &sink, fixture.to_str().unwrap_or("")])?;
        std::thread::sleep(Duration::from_millis(1600));
        let _ = Command::new("kill")
            .args(["-INT", &recorder.id().to_string()])
            .status();
        let _ = recorder.wait();
        let (_, a) = read_mono(&fixture)?;
        let (_, b) = read_mono(&recording)?;
        let corr = correlation(&a, &b, 16_000);

        // Unload mid-recording. PipeWire keeps a capture stream alive on a
        // vanished sink and feeds it silence, so the recorder detects the
        // swap two ways: the monitor source disappears from the source
        // list, and the samples recorded after the unload are silent while
        // the ones before it were not.
        let unload_path = work.join("unload.wav");
        let mut second = Command::new("pw-record")
            .args([
                "--target",
                &sink,
                "-P",
                "{ stream.capture.sink = true }",
                "--rate",
                "16000",
                "--channels",
                "1",
                "--format",
                "s16",
            ])
            .arg(&unload_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(ChildOwner::new)?;
        std::thread::sleep(Duration::from_millis(300));
        let mut player = Command::new("pw-play")
            .args(["--target", &sink])
            .arg(&fixture)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(ChildOwner::new)?;
        std::thread::sleep(Duration::from_millis(400));
        rig(repo_root, &["down", &sink])?;
        let _ = player.kill();
        let _ = player.wait();
        std::thread::sleep(Duration::from_millis(700));
        let listed = Command::new("pactl")
            .args(["list", "short", "sources"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&monitor))
            .unwrap_or(true);
        let ended = second.try_wait()?.is_some();
        let _ = Command::new("kill")
            .args(["-INT", &second.id().to_string()])
            .output();
        let _ = second.wait();
        let (_, u) = read_mono(&unload_path).unwrap_or((16_000, Vec::new()));
        let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len().max(1) as f64).sqrt();
        let n = u.len();
        let before = if n > 16_000 {
            rms(&u[4_800..9_600])
        } else {
            rms(&u)
        };
        let after = if n > 8_000 { rms(&u[n - 8_000..]) } else { 0.0 };
        let detected = ended || (!listed && before > 0.01 && after < 0.001);
        let pass = corr >= tolerance && detected;
        Ok(AudioReport {
            outcome: if pass { "pass" } else { "fail" }.into(),
            sink: sink.clone(),
            monitor: Some(monitor.clone()),
            correlation: Some(corr),
            tolerance,
            unload_detected: Some(detected),
            reason: if pass {
                None
            } else if corr < tolerance {
                Some(format!(
                    "recording correlates {corr:.3} with the fixture, below {tolerance}"
                ))
            } else {
                Some("unloading the sink was not visible to the recorder (source still listed or samples not silent)".into())
            },
        })
    })();
    let _ = rig(repo_root, &["down", &sink]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `pactl` that keeps its module table in a file, beside a `pw-play`
    /// that does nothing, on a PATH of their own.
    fn stub_pactl(dir: &Path) -> std::io::Result<()> {
        let table = dir.join("modules");
        std::fs::write(&table, "")?;
        let pactl = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
table="{table}"
case "$1 ${{2:-}}" in
  "info ") exit 0 ;;
  "load-module module-null-sink")
    id=$(( $(wc -l <"$table") + 1 ))
    printf '%s	module-null-sink	%s %s
' "$id" "$3" "$4" >>"$table"
    echo "$id" ;;
  "list short")
    case "$3" in
      modules) cat "$table" ;;
      sources) [[ -n "${{STUB_NO_MONITOR:-}}" ]] || awk '{{ sub("sink_name=", "", $3); print $1 "	" $3 ".monitor	PipeWire	s16le 1ch 16000Hz	SUSPENDED" }}' "$table" ;;
    esac ;;
  "unload-module "*) grep -v "^$2	" "$table" >"$table.next" || true; mv "$table.next" "$table" ;;
  *) echo "stub pactl: $*" >&2; exit 9 ;;
esac
"#,
            table = table.display()
        );
        std::fs::write(dir.join("pactl"), pactl)?;
        std::fs::write(
            dir.join("pw-play"),
            "#!/usr/bin/env bash
exit 0
",
        )?;
        use std::os::unix::fs::PermissionsExt;
        for name in ["pactl", "pw-play"] {
            std::fs::set_permissions(dir.join(name), std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    fn rig_with_stub(dir: &Path, args: &[&str], no_monitor: bool) -> (bool, String) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = format!(
            "{}:{}",
            dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut cmd = Command::new(rig_script(&repo));
        cmd.args(args).env("PATH", path);
        if no_monitor {
            cmd.env("STUB_NO_MONITOR", "1");
        }
        let out = cmd.output().expect("audio-rig.sh runs");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        )
    }

    #[test]
    fn down_unloads_only_the_named_sink_and_a_monitorless_up_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        stub_pactl(dir.path()).unwrap();
        let table = dir.path().join("modules");
        assert!(rig_with_stub(dir.path(), &["up", "dettivo-qa-mic-123"], false).0);
        assert!(rig_with_stub(dir.path(), &["up", "dettivo-qa-mic-1234"], false).0);
        assert!(rig_with_stub(dir.path(), &["down", "dettivo-qa-mic-123"], false).0);
        let left = std::fs::read_to_string(&table).unwrap();
        assert!(
            left.contains("sink_name=dettivo-qa-mic-1234")
                && !left.contains("sink_name=dettivo-qa-mic-123 "),
            "down took the wrong sink: {left:?}"
        );
        let (ok, err) = rig_with_stub(dir.path(), &["up", "dettivo-qa-mic-9"], true);
        assert!(!ok && err.contains("never appeared"), "{err}");
        let left = std::fs::read_to_string(&table).unwrap();
        assert!(
            !left.contains("dettivo-qa-mic-9"),
            "the module of a sink without a monitor stayed loaded: {left:?}"
        );
    }

    #[test]
    fn fixture_round_trips_and_correlates_with_itself_under_delay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.wav");
        write_fixture(&path).unwrap();
        let (rate, samples) = read_mono(&path).unwrap();
        assert_eq!(rate, 16_000);
        assert_eq!(samples.len(), 16_000);
        let mut delayed = vec![0.0; 800];
        delayed.extend_from_slice(&samples);
        assert!(correlation(&samples, &delayed, 1000) > 0.99);
        let noise: Vec<f64> = (0..16_000)
            .map(|i| if i % 7 == 0 { 0.3 } else { -0.2 })
            .collect();
        assert!(correlation(&samples, &noise, 100) < 0.5);
    }
}
