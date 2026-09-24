//! The fixture capture behind `DETTIVO_MOCK_MIC`: a WAV file delivered at
//! real time through the same `Event` stream a PipeWire capture produces,
//! so nothing above the capture layer can tell the difference.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::level::Meter;
use crate::{CaptureError, EndReason, Event, SAMPLE_RATE};

/// A running fixture capture.
pub struct MockCapture {
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

/// Reads a WAV into 16 kHz mono i16, converting channels and rate once.
pub fn read_fixture(path: &Path) -> Result<Vec<i16>, CaptureError> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| CaptureError::Fixture(format!("{}: {e}", path.display())))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels).max(1);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            if !(1..=32).contains(&spec.bits_per_sample) {
                return Err(CaptureError::Fixture(format!(
                    "{}: unsupported bits per sample {}",
                    path.display(),
                    spec.bits_per_sample
                )));
            }
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| CaptureError::Fixture(format!("{}: {e}", path.display())))?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| CaptureError::Fixture(format!("{}: {e}", path.display())))?,
    };
    let mono: Vec<f32> = samples
        .chunks(channels)
        .map(|c| c.iter().sum::<f32>() / channels as f32)
        .collect();
    let resampled: Vec<f32> = if spec.sample_rate == SAMPLE_RATE {
        mono
    } else {
        let ratio = f64::from(spec.sample_rate) / f64::from(SAMPLE_RATE);
        let out_len = (mono.len() as f64 / ratio) as usize;
        (0..out_len)
            .map(|i| {
                let pos = i as f64 * ratio;
                let a = pos.floor() as usize;
                let b = (a + 1).min(mono.len().saturating_sub(1));
                let t = (pos - a as f64) as f32;
                mono.get(a).copied().unwrap_or(0.0) * (1.0 - t)
                    + mono.get(b).copied().unwrap_or(0.0) * t
            })
            .collect()
    };
    Ok(resampled
        .into_iter()
        .map(|v| (v.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect())
}

impl MockCapture {
    /// Starts delivering `path` at real time in 20 ms chunks.
    pub fn open(path: &Path, level_interval_ms: u64) -> Result<Self, CaptureError> {
        let samples = read_fixture(path)?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::Builder::new()
            .name("dettivo-mock-mic".into())
            .spawn(move || {
                let chunk = (SAMPLE_RATE / 50) as usize;
                let mut meter = Meter::new(level_interval_ms);
                let start = Instant::now();
                let mut sent = 0usize;
                for (i, piece) in samples.chunks(chunk).enumerate() {
                    if flag.load(Ordering::Relaxed) {
                        let _ = tx.send(Event::Ended {
                            reason: EndReason::Stopped,
                        });
                        return;
                    }
                    let due = start + Duration::from_millis(i as u64 * 20);
                    let now = Instant::now();
                    if due > now {
                        std::thread::sleep(due - now);
                    }
                    if tx.send(Event::Pcm(piece.to_vec())).is_err() {
                        return;
                    }
                    for (rms, peak) in meter.push(piece) {
                        let _ = tx.send(Event::Level { rms, peak });
                    }
                    sent += piece.len();
                }
                let _ = sent;
                let _ = tx.send(Event::Ended {
                    reason: EndReason::FixtureFinished,
                });
            })
            .map_err(|e| CaptureError::Fixture(e.to_string()))?;
        Ok(Self { events: rx, stop })
    }

    /// The event stream.
    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    /// Stops delivery; the stream ends with `Stopped`.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_plays_at_real_time_with_levels_and_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for n in 0..24_000 {
            let v = (0.5
                * (2.0 * std::f64::consts::PI * 440.0 * f64::from(n) / 48_000.0).sin()
                * 32_767.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        let start = Instant::now();
        let capture = MockCapture::open(&path, 50).unwrap();
        let mut pcm = 0usize;
        let mut levels = 0usize;
        let mut ended = None;
        for event in capture.events().iter() {
            match event {
                Event::Pcm(chunk) => pcm += chunk.len(),
                Event::Level { rms, .. } => {
                    assert!(rms > 0.2);
                    levels += 1;
                }
                Event::Ended { reason } => {
                    ended = Some(reason);
                    break;
                }
                Event::DeviceChanged { .. } => panic!("fixture never swaps"),
            }
        }
        assert_eq!(ended, Some(EndReason::FixtureFinished));
        assert!(
            (7_900..=8_000).contains(&pcm),
            "{pcm} samples at 16 kHz for 0.5 s"
        );
        assert!(levels >= 9, "{levels}");
        assert!(
            start.elapsed() >= Duration::from_millis(450),
            "delivered at real time"
        );
    }
}
