//! The always-ready microphone's ring (ADR 0092): the last ten seconds of
//! 16 kHz mono capture in memory, each chunk stamped with the monotonic
//! time its first sample was recorded, so a dictation can begin before
//! the press that started it. Nothing here touches the disk.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::SAMPLE_RATE;

/// How much audio the ring keeps.
pub const RING_CAPACITY: Duration = Duration::from_secs(10);

/// One delivered chunk and when its first sample was captured.
struct Chunk {
    at: Instant,
    pcm: Box<[i16]>,
}

/// The ring: whole chunks, oldest first, never more than its capacity.
pub struct AudioRing {
    chunks: VecDeque<Chunk>,
    samples: usize,
    cap: usize,
}

fn samples_in(d: Duration) -> usize {
    (d.as_micros() * u128::from(SAMPLE_RATE) / 1_000_000) as usize
}

fn duration_of(samples: usize) -> Duration {
    Duration::from_micros(samples as u64 * 1_000_000 / u64::from(SAMPLE_RATE))
}

impl AudioRing {
    /// An empty ring that keeps `cap` of audio.
    pub fn new(cap: Duration) -> Self {
        Self {
            chunks: VecDeque::new(),
            samples: 0,
            cap: samples_in(cap),
        }
    }

    /// Appends a chunk whose last sample was captured at `received`. The
    /// oldest chunks leave once the ring holds more than its capacity
    /// without them.
    pub fn push(&mut self, received: Instant, pcm: &[i16]) {
        if pcm.is_empty() {
            return;
        }
        let at = received
            .checked_sub(duration_of(pcm.len()))
            .unwrap_or(received);
        self.samples += pcm.len();
        self.chunks.push_back(Chunk {
            at,
            pcm: pcm.into(),
        });
        while let Some(front) = self.chunks.front() {
            if self.samples - front.pcm.len() < self.cap {
                break;
            }
            self.samples -= front.pcm.len();
            self.chunks.pop_front();
        }
    }

    /// Every chunk recorded from `since` onward, each with the time its
    /// last sample was captured, the first trimmed to the sample;
    /// everything the ring holds when `since` is older. Chunks stay
    /// apart so a gap in the capture keeps its place in time.
    pub fn since(&self, since: Instant) -> Vec<(Instant, Vec<i16>)> {
        let mut out = Vec::new();
        for chunk in &self.chunks {
            let end = chunk.at + duration_of(chunk.pcm.len());
            if end <= since {
                continue;
            }
            let skip = samples_in(since.saturating_duration_since(chunk.at));
            out.push((end, chunk.pcm[skip.min(chunk.pcm.len())..].to_vec()));
        }
        out
    }

    /// Forgets everything (a moved default source).
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The samples of `chunks`, in order.
    fn samples(chunks: Vec<(Instant, Vec<i16>)>) -> Vec<i16> {
        chunks.into_iter().flat_map(|(_, pcm)| pcm).collect()
    }

    /// Chunks of 20 ms whose samples count up from 0, received at
    /// `t0 + 20 ms × (i + 1)`.
    fn filled(t0: Instant, chunks: usize, cap: Duration) -> AudioRing {
        let mut ring = AudioRing::new(cap);
        for i in 0..chunks {
            let pcm: Vec<i16> = (0..320).map(|s| (i * 320 + s) as i16).collect();
            ring.push(t0 + Duration::from_millis(20 * (i as u64 + 1)), &pcm);
        }
        ring
    }

    #[test]
    fn since_starts_at_the_exact_sample() {
        let t0 = Instant::now();
        let ring = filled(t0, 50, RING_CAPACITY);
        // 450 ms before a press at 1000 ms is sample 8800 (550 ms in).
        let chunks = ring.since(t0 + Duration::from_millis(1000) - Duration::from_millis(450));
        assert_eq!(
            chunks[0].0,
            t0 + Duration::from_millis(560),
            "its own stamp"
        );
        assert_eq!(chunks.last().unwrap().0, t0 + Duration::from_millis(1000));
        let taken = samples(chunks);
        assert_eq!(taken.first(), Some(&8800));
        assert_eq!(taken.len(), 16_000 - 8800);
        assert_eq!(taken.last(), Some(&15_999));
    }

    #[test]
    fn since_older_than_the_ring_returns_everything_and_newer_returns_nothing() {
        let t0 = Instant::now() + Duration::from_secs(1);
        let ring = filled(t0, 10, RING_CAPACITY);
        assert_eq!(samples(ring.since(t0 - Duration::from_secs(1))).len(), 3200);
        assert!(ring.since(t0 + Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn the_ring_keeps_its_capacity_and_clears() {
        let t0 = Instant::now();
        let mut ring = filled(t0, 100, Duration::from_millis(500));
        let all = samples(ring.since(t0));
        assert_eq!(all.len(), 8000, "500 ms at 16 kHz");
        assert_eq!(all.first(), Some(&(75 * 320)), "the oldest chunks left");
        ring.clear();
        assert!(ring.since(t0).is_empty());
    }
}
