//! Rate conversion to 16 kHz for the decoder: a windowed-sinc resampler
//! (`rubato`) fed in fixed blocks, with the filter delay trimmed at the
//! start and the tail flushed at the end so the output holds exactly the
//! frames the input duration implies.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, WindowFunction};

use crate::SAMPLE_RATE;

/// Input frames per resampler call.
const CHUNK: usize = 1024;

/// Converts mono samples at one rate to [`SAMPLE_RATE`].
pub struct ToSixteenK {
    inner: Async<f32>,
    ratio: f64,
    pending: Vec<f32>,
    delay_left: usize,
    in_frames: u64,
    out_frames: u64,
}

impl ToSixteenK {
    /// A resampler from `source_rate`; `None` when no conversion is needed.
    pub fn new(source_rate: u32) -> Result<Option<Self>, String> {
        if source_rate == SAMPLE_RATE {
            return Ok(None);
        }
        let ratio = f64::from(SAMPLE_RATE) / f64::from(source_rate.max(1));
        let params = SincInterpolationParameters::new(256, WindowFunction::BlackmanHarris2);
        let inner = Async::<f32>::new_sinc(ratio, 1.0, &params, CHUNK, 1, FixedAsync::Input)
            .map_err(|e| format!("resampler: {e}"))?;
        let delay_left = inner.output_delay();
        Ok(Some(Self {
            inner,
            ratio,
            pending: Vec::with_capacity(CHUNK * 2),
            delay_left,
            in_frames: 0,
            out_frames: 0,
        }))
    }

    /// Feeds mono samples; every full block is converted onto `out`.
    pub fn push(&mut self, mono: &[f32], out: &mut Vec<f32>) -> Result<(), String> {
        self.pending.extend_from_slice(mono);
        let mut start = 0;
        while self.pending.len() - start >= CHUNK {
            let block: Vec<f32> = self.pending[start..start + CHUNK].to_vec();
            self.in_frames += CHUNK as u64;
            self.run(&block, None, out)?;
            start += CHUNK;
        }
        self.pending.drain(..start);
        Ok(())
    }

    /// Converts the remaining partial block and flushes the filter so the
    /// output length matches the input duration.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), String> {
        let rest: Vec<f32> = std::mem::take(&mut self.pending);
        self.in_frames += rest.len() as u64;
        let expected = (self.in_frames as f64 * self.ratio).round() as u64;
        let mut padded = rest;
        padded.resize(CHUNK, 0.0);
        self.run(&padded, Some(self.in_frames as usize % CHUNK), out)?;
        let silence = vec![0.0f32; CHUNK];
        let mut rounds = 0;
        while self.out_frames < expected && rounds < 64 {
            self.run(&silence, Some(0), out)?;
            rounds += 1;
        }
        let excess = self.out_frames.saturating_sub(expected) as usize;
        if excess > 0 && excess <= out.len() {
            out.truncate(out.len() - excess);
            self.out_frames = expected;
        }
        Ok(())
    }

    fn run(
        &mut self,
        block: &[f32],
        partial: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<(), String> {
        let input = InterleavedSlice::new(block, 1, block.len()).map_err(|e| format!("{e:?}"))?;
        let indexing = partial.map(|n| Indexing::new().partial_len(n));
        let produced = self
            .inner
            .process(&input, indexing.as_ref())
            .map_err(|e| format!("resampler: {e}"))?
            .take_data();
        let skip = self.delay_left.min(produced.len());
        self.delay_left -= skip;
        out.extend_from_slice(&produced[skip..]);
        self.out_frames += (produced.len() - skip) as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_keeps_its_length_and_level_across_the_rate_change() {
        let rate = 44_100u32;
        let seconds = 1.5;
        let n = (f64::from(rate) * seconds) as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect();
        let mut r = ToSixteenK::new(rate).unwrap().expect("a conversion");
        let mut out = Vec::new();
        for block in tone.chunks(3000) {
            r.push(block, &mut out).unwrap();
        }
        r.finish(&mut out).unwrap();
        let expected = (f64::from(SAMPLE_RATE) * seconds).round() as usize;
        assert_eq!(out.len(), expected, "exact frame count");
        let rms = (out[2000..]
            .iter()
            .map(|v| f64::from(*v) * f64::from(*v))
            .sum::<f64>()
            / (out.len() - 2000) as f64)
            .sqrt();
        assert!((rms - 0.3536).abs() < 0.02, "rms {rms}");
        assert!(ToSixteenK::new(SAMPLE_RATE).unwrap().is_none());
    }
}
