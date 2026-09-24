//! Cutting audio into chunks: fixed windows of `chunk_seconds`, each
//! starting `overlap_seconds` before the previous one ended, with the cut
//! moved to the quietest point inside the last `safety_margin_seconds` of
//! the window so a chunk rarely ends mid-word.

use crate::source::AudioSource;
use crate::{SAMPLE_RATE, Settings};

/// The window the quiet-point search measures, in samples (50 ms).
pub const QUIET_WINDOW: usize = 800;

/// One chunk, in samples of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    /// Zero-based index.
    pub index: u32,
    /// First sample.
    pub start: u64,
    /// One past the last sample.
    pub end: u64,
}

impl Chunk {
    /// The start on the audio's clock.
    pub fn start_ms(&self) -> u64 {
        self.start * 1000 / SAMPLE_RATE
    }

    /// The end on the audio's clock.
    pub fn end_ms(&self) -> u64 {
        self.end * 1000 / SAMPLE_RATE
    }
}

/// The offset of the centre of the quietest `QUIET_WINDOW` inside
/// `samples` (the latest one on a tie, so the chunk stays as long as it
/// can); the end of the slice when it is shorter than a window.
pub fn quietest_point(samples: &[i16]) -> usize {
    if samples.len() < QUIET_WINDOW {
        return samples.len();
    }
    let mut best = (f64::INFINITY, samples.len());
    let mut start = 0;
    while start + QUIET_WINDOW <= samples.len() {
        let level = crate::filters::rms(&samples[start..start + QUIET_WINDOW]);
        if level <= best.0 {
            best = (level, start + QUIET_WINDOW / 2);
        }
        start += QUIET_WINDOW / 2;
    }
    best.1
}

/// Plans the chunks of `source` under `settings`; a window that cannot
/// hold the overlap and the margin is refused naming the keys.
pub fn plan(source: &mut dyn AudioSource, settings: &Settings) -> Result<Vec<Chunk>, String> {
    let chunk = settings.chunk_seconds * SAMPLE_RATE;
    let overlap = settings.overlap_seconds * SAMPLE_RATE;
    let margin = settings.safety_margin_seconds * SAMPLE_RATE;
    if chunk <= overlap + margin {
        return Err(format!(
            "transcribe.chunk_seconds ({}) must exceed overlap_seconds ({}) plus safety_margin_seconds ({})",
            settings.chunk_seconds, settings.overlap_seconds, settings.safety_margin_seconds
        ));
    }
    let len = source.len();
    let mut chunks = Vec::new();
    let mut start = 0u64;
    loop {
        let nominal = start + chunk;
        if nominal >= len {
            chunks.push(Chunk {
                index: chunks.len() as u32,
                start,
                end: len,
            });
            return Ok(chunks);
        }
        let region_start = nominal - margin;
        let region = source.read(region_start, nominal)?;
        let mut end = region_start + quietest_point(&region) as u64;
        if end <= start + overlap {
            end = nominal;
        }
        chunks.push(Chunk {
            index: chunks.len() as u32,
            start,
            end,
        });
        start = end - overlap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: u64, amplitude: i16) -> Vec<i16> {
        (0..seconds * SAMPLE_RATE)
            .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
            .collect()
    }

    #[test]
    fn a_short_file_is_one_chunk_and_bad_settings_are_refused() {
        let mut pcm = tone(30, 1000);
        let chunks = plan(&mut pcm, &Settings::default()).unwrap();
        assert_eq!(
            chunks,
            vec![Chunk {
                index: 0,
                start: 0,
                end: 30 * SAMPLE_RATE
            }]
        );
        let bad = Settings {
            chunk_seconds: 7,
            overlap_seconds: 2,
            safety_margin_seconds: 5,
            ..Settings::default()
        };
        let err = plan(&mut pcm, &bad).unwrap_err();
        assert!(err.contains("chunk_seconds (7)"), "{err}");
    }

    #[test]
    fn windows_overlap_by_two_seconds_and_cut_at_the_quiet_point_in_the_margin() {
        // 12 minutes of tone with a 200 ms gap of silence every 6 s.
        let mut pcm = tone(720, 2000);
        for rep in 0..120u64 {
            let gap = (rep * 6 + 5) * SAMPLE_RATE;
            for s in &mut pcm[gap as usize..(gap + SAMPLE_RATE / 5) as usize] {
                *s = 0;
            }
        }
        let chunks = plan(
            &mut pcm,
            &Settings {
                chunk_seconds: 300,
                ..Settings::default()
            },
        )
        .unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].start, 0);
        // The first window's margin is 295 s to 300 s; the gap inside it
        // starts at 299 s, so the cut lands in it.
        let cut = chunks[0].end as f64 / SAMPLE_RATE as f64;
        assert!((299.0..=299.2).contains(&cut), "cut at {cut}");
        assert_eq!(chunks[1].start, chunks[0].end - 2 * SAMPLE_RATE);
        assert_eq!(chunks[1].index, 1);
        let second = chunks[1].end as f64 / SAMPLE_RATE as f64;
        assert!((593.0..=593.2).contains(&second), "second cut at {second}");
        assert_eq!(chunks[2].end, 720 * SAMPLE_RATE);
        assert_eq!(chunks[2].start_ms(), (chunks[2].start * 1000) / SAMPLE_RATE);
    }

    #[test]
    fn a_margin_without_a_quiet_point_still_ends_late_in_the_window() {
        let mut pcm = tone(400, 2000);
        let chunks = plan(
            &mut pcm,
            &Settings {
                chunk_seconds: 300,
                ..Settings::default()
            },
        )
        .unwrap();
        let cut = chunks[0].end as f64 / SAMPLE_RATE as f64;
        assert!((295.0..=300.0).contains(&cut), "cut at {cut}");
        assert_eq!(quietest_point(&[1, 2, 3]), 3);
    }
}
