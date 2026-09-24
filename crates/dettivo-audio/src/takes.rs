//! Takes: each capture run of a session is one WAV file, 16 kHz mono
//! 16-bit, named `microphone.wav`, `microphone-2.wav` and so on (or
//! `system.wav` for a meeting's system track), with a sidecar recording
//! every take's start offset on the session clock and whether a device
//! swap left a gap before it (FR-A2, FR-A4). Two writers can share one
//! clock so a meeting's tracks carry offsets on the same time base, and a
//! take whose writer died before `finalize` (a killed daemon) is repaired
//! from the bytes on disk.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// The sidecar of the microphone takes.
pub const SIDECAR: &str = "takes.json";

/// One take in the sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Take {
    /// 1-based index.
    pub index: u32,
    /// File name relative to the artifact directory.
    pub file: String,
    /// Milliseconds since the session started when this take began.
    pub start_offset_ms: u64,
    /// Nanoseconds since the session started when this take began (the
    /// same instant as `start_offset_ms`, at the clock's precision).
    #[serde(default)]
    pub start_offset_ns: u64,
    /// True when capture was interrupted before this take (device swap).
    pub gap_before: bool,
    /// Samples written.
    pub samples: u64,
}

/// The sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Takes {
    /// Takes in order.
    pub takes: Vec<Take>,
}

impl Takes {
    /// Milliseconds of audio across every take.
    pub fn duration_ms(&self) -> u64 {
        self.takes.iter().map(|t| t.samples).sum::<u64>() * 1000 / u64::from(crate::SAMPLE_RATE)
    }
}

/// Writes takes into one artifact directory.
pub struct TakeWriter {
    dir: PathBuf,
    prefix: String,
    sidecar: String,
    takes: Takes,
    current: Option<(hound::WavWriter<std::io::BufWriter<std::fs::File>>, u64)>,
    started: Instant,
}

impl TakeWriter {
    /// Prepares `dir` (created when missing) for `microphone*.wav` takes;
    /// the session clock starts now.
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        Self::with_prefix(dir, "microphone", Instant::now())
    }

    /// A writer for `<prefix>.wav`, `<prefix>-2.wav`, ... whose offsets
    /// count from `started`, so two tracks share one clock. The sidecar is
    /// `takes.json` for the microphone and `<prefix>-takes.json` otherwise.
    pub fn with_prefix(dir: &Path, prefix: &str, started: Instant) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let sidecar = if prefix == "microphone" {
            SIDECAR.to_string()
        } else {
            format!("{prefix}-takes.json")
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            prefix: prefix.to_string(),
            sidecar,
            takes: Takes::default(),
            current: None,
            started,
        })
    }

    fn spec() -> hound::WavSpec {
        hound::WavSpec {
            channels: 1,
            sample_rate: crate::SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }
    }

    /// Starts a new take; `gap_before` marks a device swap.
    pub fn start_take(&mut self, gap_before: bool) -> std::io::Result<&Take> {
        self.start_take_at(Instant::now(), gap_before)
    }

    /// Starts a new take whose first sample was captured at `at`: the
    /// offset counts from the session clock to that instant rather than
    /// to the writer's creation, so a source that opened before its
    /// writer keeps the origin its samples actually have.
    pub fn start_take_at(&mut self, at: Instant, gap_before: bool) -> std::io::Result<&Take> {
        self.finish_take()?;
        let index = self.takes.takes.len() as u32 + 1;
        let file = if index == 1 {
            format!("{}.wav", self.prefix)
        } else {
            format!("{}-{index}.wav", self.prefix)
        };
        let writer = hound::WavWriter::create(self.dir.join(&file), Self::spec())
            .map_err(std::io::Error::other)?;
        let elapsed = at.saturating_duration_since(self.started);
        self.takes.takes.push(Take {
            index,
            file,
            start_offset_ms: elapsed.as_millis() as u64,
            start_offset_ns: elapsed.as_nanos() as u64,
            gap_before,
            samples: 0,
        });
        self.current = Some((writer, 0));
        self.write_sidecar()?;
        Ok(self.takes.takes.last().expect("just pushed"))
    }

    /// Appends samples to the current take (starts one when none is open).
    pub fn write(&mut self, samples: &[i16]) -> std::io::Result<()> {
        if self.current.is_none() {
            self.start_take(false)?;
        }
        let (writer, count) = self.current.as_mut().expect("opened above");
        for &s in samples {
            writer.write_sample(s).map_err(std::io::Error::other)?;
        }
        *count += samples.len() as u64;
        Ok(())
    }

    /// Flushes the current take's samples to disk without closing it, and
    /// records the count so far in the sidecar (a checkpoint).
    pub fn flush(&mut self) -> std::io::Result<()> {
        if let Some((writer, count)) = self.current.as_mut() {
            writer.flush().map_err(std::io::Error::other)?;
            if let Some(take) = self.takes.takes.last_mut() {
                take.samples = *count;
            }
            self.write_sidecar()?;
        }
        Ok(())
    }

    /// Closes the current take and records its length.
    pub fn finish_take(&mut self) -> std::io::Result<()> {
        if let Some((writer, count)) = self.current.take() {
            writer.finalize().map_err(std::io::Error::other)?;
            if let Some(take) = self.takes.takes.last_mut() {
                take.samples = count;
            }
            self.write_sidecar()?;
        }
        Ok(())
    }

    /// Closes everything and returns the sidecar.
    pub fn finish(mut self) -> std::io::Result<Takes> {
        self.finish_take()?;
        Ok(self.takes.clone())
    }

    /// The sidecar so far.
    pub fn takes(&self) -> &Takes {
        &self.takes
    }

    /// True while a take is open.
    pub fn is_open(&self) -> bool {
        self.current.is_some()
    }

    /// Removes every take file and the sidecar (a dictation session that
    /// does not keep its audio).
    pub fn discard(dir: &Path) -> std::io::Result<()> {
        if let Ok(text) = std::fs::read_to_string(dir.join(SIDECAR)) {
            if let Ok(takes) = serde_json::from_str::<Takes>(&text) {
                for take in takes.takes {
                    // Only a plain file name inside `dir` is ever removed; a
                    // sidecar naming a path is left alone.
                    let name = Path::new(&take.file);
                    let plain = name.components().count() == 1
                        && name.file_name().is_some_and(|n| n == take.file.as_str())
                        && take.file.ends_with(".wav");
                    if plain {
                        let _ = std::fs::remove_file(dir.join(&take.file));
                    }
                }
            }
        }
        let _ = std::fs::remove_file(dir.join(SIDECAR));
        Ok(())
    }

    fn write_sidecar(&self) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(&self.takes).map_err(std::io::Error::other)?;
        std::fs::write(self.dir.join(&self.sidecar), text)
    }
}

/// Reads a sidecar.
pub fn read_sidecar(path: &Path) -> std::io::Result<Takes> {
    serde_json::from_str(&std::fs::read_to_string(path)?).map_err(std::io::Error::other)
}

/// Repairs a 16-bit mono WAV whose writer never finalised it (the daemon
/// was killed): the RIFF and data sizes are rewritten from the file
/// length so every sample on disk reads back. Returns the sample count;
/// a file that is not a WAV of that shape is left alone and reported.
pub fn repair_wav(path: &Path) -> std::io::Result<u64> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let len = file.metadata()?.len();
    let mut header = [0u8; 44];
    if len < 44 {
        return Err(std::io::Error::other("shorter than a WAV header"));
    }
    file.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" || &header[36..40] != b"data" {
        return Err(std::io::Error::other("not a canonical 16-bit WAV"));
    }
    let data = len - 44;
    let data = data - data % 2;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&((data + 36) as u32).to_le_bytes())?;
    file.seek(SeekFrom::Start(40))?;
    file.write_all(&(data as u32).to_le_bytes())?;
    file.flush()?;
    Ok(data / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_are_numbered_with_offsets_gaps_and_a_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = TakeWriter::new(dir.path()).unwrap();
        writer.write(&[1, 2, 3, 4]).unwrap();
        writer.finish_take().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        writer.start_take(true).unwrap();
        writer.write(&[5, 6]).unwrap();
        let takes = writer.finish().unwrap();
        assert_eq!(takes.takes.len(), 2);
        assert_eq!(takes.takes[0].file, "microphone.wav");
        assert_eq!(takes.takes[0].samples, 4);
        assert!(!takes.takes[0].gap_before);
        assert_eq!(takes.takes[1].file, "microphone-2.wav");
        assert!(takes.takes[1].gap_before);
        assert!(takes.takes[1].start_offset_ms >= 20);
        assert!(takes.takes[1].start_offset_ns >= 20_000_000);
        let reader = hound::WavReader::open(dir.path().join("microphone-2.wav")).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.len(), 2);
        let sidecar = read_sidecar(&dir.path().join("takes.json")).unwrap();
        assert_eq!(sidecar, takes);
        TakeWriter::discard(dir.path()).unwrap();
        assert!(!dir.path().join("microphone.wav").exists());
        assert!(!dir.path().join("takes.json").exists());
    }

    #[test]
    fn two_tracks_share_a_clock_and_a_killed_take_is_repaired() {
        let dir = tempfile::tempdir().unwrap();
        let clock = Instant::now();
        let mut mic = TakeWriter::with_prefix(dir.path(), "microphone", clock).unwrap();
        let mut sys = TakeWriter::with_prefix(dir.path(), "system", clock).unwrap();
        mic.write(&[1; 800]).unwrap();
        sys.write(&[2; 800]).unwrap();
        let (m, s) = (
            mic.takes().takes[0].start_offset_ms,
            sys.takes().takes[0].start_offset_ms,
        );
        assert!(m.abs_diff(s) < 50, "{m} vs {s}");
        assert_eq!(sys.takes().takes[0].file, "system.wav");
        // A checkpoint flushes the samples and the header; samples written
        // after it and before a kill sit past the header's length until
        // the repair extends it from the file length.
        sys.flush().unwrap();
        let sidecar = read_sidecar(&dir.path().join("system-takes.json")).unwrap();
        assert_eq!(sidecar.takes[0].samples, 800);
        let killed = dir.path().join("killed.wav");
        std::fs::copy(dir.path().join("system.wav"), &killed).unwrap();
        let mut bytes = std::fs::read(&killed).unwrap();
        bytes.extend(std::iter::repeat_n(0u8, 200));
        std::fs::write(&killed, bytes).unwrap();
        assert_eq!(hound::WavReader::open(&killed).unwrap().len(), 800);
        assert_eq!(repair_wav(&killed).unwrap(), 900);
        assert_eq!(hound::WavReader::open(&killed).unwrap().len(), 900);
        assert!(repair_wav(&dir.path().join("system-takes.json")).is_err());
        assert_eq!(sys.finish().unwrap().duration_ms(), 50);
        assert_eq!(mic.finish().unwrap().duration_ms(), 50);
    }
}
