//! R1, R2 and R3 against a real PipeWire session: a null sink fed by
//! `pw-play` is captured through the sink's monitor, the take matches the
//! fixture, levels flow, and unloading the default source mid-capture is
//! reported within a second. Skipped (and said so) where PipeWire is not
//! running, as in the CI container; the fixture path covers R5 there.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use dettivo_audio::service::AudioService;
use dettivo_audio::takes::TakeWriter;
use dettivo_audio::{EndReason, Event, Target};

fn pipewire_running() -> bool {
    Command::new("pactl")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

struct Sink {
    name: String,
    module: String,
}

impl Sink {
    fn load(name: &str) -> Self {
        let out = Command::new("pactl")
            .args([
                "load-module",
                "module-null-sink",
                &format!("sink_name={name}"),
                &format!("sink_properties=device.description={name}"),
            ])
            .output()
            .expect("pactl");
        assert!(out.status.success(), "module-null-sink failed");
        let module = String::from_utf8_lossy(&out.stdout).trim().to_string();
        std::thread::sleep(Duration::from_millis(300));
        Self {
            name: name.to_string(),
            module,
        }
    }

    fn unload(&mut self) {
        if !self.module.is_empty() {
            let _ = Command::new("pactl")
                .args(["unload-module", &self.module])
                .output();
            self.module.clear();
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        self.unload();
    }
}

fn write_fixture(path: &std::path::Path) -> Vec<i16> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut samples = Vec::new();
    for n in 0..16_000u32 {
        let t = f64::from(n) / 16_000.0;
        let v = 0.4 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
            + 0.3 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
        let s = (v * 32_767.0) as i16;
        w.write_sample(s).unwrap();
        samples.push(s);
    }
    w.finalize().unwrap();
    samples
}

fn correlation(a: &[i16], b: &[i16], max_lag: usize) -> f64 {
    let fa: Vec<f64> = a.iter().map(|&x| f64::from(x)).collect();
    let fb: Vec<f64> = b.iter().map(|&x| f64::from(x)).collect();
    let norm = |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
    let (na, nb) = (norm(&fa), norm(&fb));
    let mut best = 0.0f64;
    for lag in 0..=max_lag {
        let n = fa.len().min(fb.len().saturating_sub(lag));
        if n == 0 {
            break;
        }
        let dot: f64 = (0..n).map(|i| fa[i] * fb[i + lag]).sum();
        best = best.max(dot / (na * nb));
    }
    best
}

#[test]
fn a_fixture_played_into_a_null_sink_is_captured_with_levels_and_written_as_a_take() {
    if !pipewire_running() {
        eprintln!("skip: PipeWire is not running (pactl info failed)");
        return;
    }
    let sink = Sink::load(&format!("dettivo-audio-r1-{}", std::process::id()));
    let dir = tempfile::tempdir().unwrap();
    let fixture = dir.path().join("fixture.wav");
    let reference = write_fixture(&fixture);

    let graph = AudioService::start().expect("audio service");
    assert!(
        graph.node_named(&sink.name).is_some_and(|n| n.is_sink),
        "sink is in the graph"
    );
    let capture = graph
        .open(Target::Node(sink.name.clone()), 50)
        .expect("capture");
    std::thread::sleep(Duration::from_millis(300));
    let mut player = Command::new("pw-play")
        .args(["--target", &sink.name])
        .arg(&fixture)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("pw-play");
    let mut writer = TakeWriter::new(dir.path()).unwrap();
    let mut levels = 0usize;
    let mut loud_levels = 0usize;
    let mut pcm: Vec<i16> = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(2_500);
    while Instant::now() < deadline {
        match capture.events().recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Pcm(chunk)) => {
                writer.write(&chunk).unwrap();
                pcm.extend(chunk);
            }
            Ok(Event::Level { rms, peak }) => {
                assert!((0.0..=1.0).contains(&rms) && (0.0..=1.0).contains(&peak));
                levels += 1;
                if rms > 0.05 {
                    loud_levels += 1;
                }
            }
            Ok(Event::Ended { reason }) => panic!("capture ended early: {reason:?}"),
            Ok(Event::DeviceChanged { .. }) => panic!("pinned capture never swaps"),
            Err(_) => {}
        }
    }
    let _ = player.kill();
    let _ = player.wait();
    capture.stop();
    let takes = writer.finish().unwrap();
    assert_eq!(takes.takes.len(), 1);
    assert!(pcm.len() > 16_000, "captured {} samples", pcm.len());
    let corr = correlation(&reference, &pcm, 24_000);
    assert!(corr > 0.99, "take correlates {corr:.3} with the fixture");
    assert!(levels >= 20, "{levels} level samples");
    assert!(loud_levels >= 10, "{loud_levels} loud level samples");
    let reader = hound::WavReader::open(dir.path().join("microphone.wav")).unwrap();
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(reader.spec().channels, 1);
}

#[test]
fn unloading_the_default_source_is_reported_within_a_second() {
    if !pipewire_running() {
        eprintln!("skip: PipeWire is not running (pactl info failed)");
        return;
    }
    let previous = Command::new("pactl")
        .arg("get-default-source")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut sink = Sink::load(&format!("dettivo-audio-r2-{}", std::process::id()));
    let set = Command::new("pactl")
        .args(["set-default-source", &format!("{}.monitor", sink.name)])
        .status()
        .expect("pactl");
    assert!(set.success());
    std::thread::sleep(Duration::from_millis(300));

    let graph = AudioService::start().expect("audio service");
    assert_eq!(
        graph.snapshot().default_source.as_deref(),
        Some(sink.name.as_str()),
        "the default source metadata names the sink"
    );
    let capture = graph.open(Target::Default, 50).expect("capture");
    std::thread::sleep(Duration::from_millis(300));
    let unloaded_at = Instant::now();
    sink.unload();
    let mut changed = None;
    let deadline = unloaded_at + Duration::from_secs(3);
    while Instant::now() < deadline {
        match capture.events().recv_timeout(Duration::from_millis(100)) {
            Ok(Event::DeviceChanged { from, to }) => {
                changed = Some((from, to, unloaded_at.elapsed()));
                break;
            }
            Ok(Event::Ended {
                reason: EndReason::NoSource,
                ..
            }) => {
                changed = Some((None, None, unloaded_at.elapsed()));
                break;
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    capture.stop();
    if !previous.is_empty() {
        let _ = Command::new("pactl")
            .args(["set-default-source", &previous])
            .status();
    }
    let (from, to, elapsed) = changed.expect("a DeviceChanged or NoSource event");
    assert!(
        elapsed < Duration::from_secs(1),
        "reported after {elapsed:?}"
    );
    assert_eq!(from.as_deref(), Some(sink.name.as_str()));
    assert_ne!(to.as_deref(), Some(sink.name.as_str()));
}

#[test]
fn a_pinned_device_that_is_absent_is_refused_by_name() {
    if !pipewire_running() {
        eprintln!("skip: PipeWire is not running (pactl info failed)");
        return;
    }
    let graph = AudioService::start().expect("audio service");
    let err = graph
        .open(Target::Node("dettivo-no-such-device".into()), 50)
        .unwrap_err();
    assert_eq!(
        err,
        dettivo_audio::CaptureError::UnknownDevice("dettivo-no-such-device".into())
    );
}

/// R5: the fixture path delivers the same fixture through the same event
/// stream, everywhere, PipeWire or not.
#[test]
fn the_fixture_path_matches_the_fixture_without_pipewire() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = dir.path().join("fixture.wav");
    let reference = write_fixture(&fixture);
    let capture = dettivo_audio::mock::MockCapture::open(&fixture, 50).expect("fixture capture");
    let mut writer = TakeWriter::new(dir.path()).unwrap();
    let mut pcm: Vec<i16> = Vec::new();
    let mut levels = 0usize;
    for event in capture.events().iter() {
        match event {
            Event::Pcm(chunk) => {
                writer.write(&chunk).unwrap();
                pcm.extend(chunk);
            }
            Event::Level { .. } => levels += 1,
            Event::Ended { reason } => {
                assert_eq!(reason, EndReason::FixtureFinished);
                break;
            }
            Event::DeviceChanged { .. } => panic!("fixtures never swap"),
        }
    }
    writer.finish().unwrap();
    assert_eq!(pcm.len(), reference.len());
    assert!(correlation(&reference, &pcm, 0) > 0.999);
    assert!(levels >= 19, "{levels}");
    if !pipewire_running() {
        eprintln!(
            "note: PipeWire is not running here; the rig tests were skipped and this fixture path stands in"
        );
    }
}
