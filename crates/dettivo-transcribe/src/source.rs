//! Where a job reads its samples: a slice in memory (tests, short takes)
//! or a 16 kHz mono WAV on disk read range by range, so an hour of audio
//! never sits in memory whole.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// 16 kHz mono samples addressable by range.
pub trait AudioSource {
    /// Total samples.
    fn len(&self) -> u64;
    /// True when there are no samples.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The samples in `[start, end)`, clamped to the end.
    fn read(&mut self, start: u64, end: u64) -> Result<Vec<i16>, String>;
}

impl AudioSource for Vec<i16> {
    fn len(&self) -> u64 {
        Vec::len(self) as u64
    }

    fn read(&mut self, start: u64, end: u64) -> Result<Vec<i16>, String> {
        let end = end.min(Vec::len(self) as u64) as usize;
        let start = (start as usize).min(end);
        Ok(self[start..end].to_vec())
    }
}

/// A 16 kHz mono 16-bit WAV read by range.
pub struct WavSource {
    reader: hound::WavReader<BufReader<File>>,
    len: u64,
}

impl WavSource {
    /// Opens `path`; refuses anything but 16 kHz mono 16-bit.
    pub fn open(path: &Path) -> Result<Self, String> {
        let reader =
            hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != crate::SAMPLE_RATE as u32
            || spec.bits_per_sample != 16
        {
            return Err(format!(
                "{}: not 16 kHz mono 16-bit ({} Hz, {} channels, {} bits)",
                path.display(),
                spec.sample_rate,
                spec.channels,
                spec.bits_per_sample
            ));
        }
        let len = u64::from(reader.len());
        Ok(Self { reader, len })
    }
}

impl AudioSource for WavSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn read(&mut self, start: u64, end: u64) -> Result<Vec<i16>, String> {
        let end = end.min(self.len);
        let start = start.min(end);
        self.reader
            .seek(u32::try_from(start).map_err(|_| "audio too long to address".to_string())?)
            .map_err(|e| e.to_string())?;
        self.reader
            .samples::<i16>()
            .take((end - start) as usize)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_is_read_by_range_and_the_wrong_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..100i16 {
            w.write_sample(i).unwrap();
        }
        w.finalize().unwrap();
        let mut source = WavSource::open(&path).unwrap();
        assert_eq!(source.len(), 100);
        assert_eq!(source.read(10, 13).unwrap(), vec![10, 11, 12]);
        assert_eq!(source.read(98, 200).unwrap(), vec![98, 99]);
        assert_eq!(source.read(5, 7).unwrap(), vec![5, 6], "seeks backwards");
        let stereo = dir.path().join("s.wav");
        let w = hound::WavWriter::create(
            &stereo,
            hound::WavSpec {
                channels: 2,
                ..spec
            },
        )
        .unwrap();
        w.finalize().unwrap();
        assert!(
            WavSource::open(&stereo)
                .err()
                .expect("refused")
                .contains("2 channels")
        );
        let mut memory = vec![1i16, 2, 3];
        assert_eq!(memory.read(1, 9).unwrap(), vec![2, 3]);
    }
}
