//! Meter samples: RMS and peak over a fixed window of 16-bit samples.

/// Accumulates samples and emits one `(rms, peak)` per window.
#[derive(Debug)]
pub struct Meter {
    window: usize,
    count: usize,
    sum_squares: f64,
    peak: f32,
}

impl Meter {
    /// A meter whose window is `interval_ms` at 16 kHz.
    pub fn new(interval_ms: u64) -> Self {
        let window = (crate::SAMPLE_RATE as u64 * interval_ms.max(1) / 1000).max(1) as usize;
        Self {
            window,
            count: 0,
            sum_squares: 0.0,
            peak: 0.0,
        }
    }

    /// Feeds samples; returns every completed window's `(rms, peak)`.
    pub fn push(&mut self, samples: &[i16]) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        for &s in samples {
            let v = f64::from(s) / 32_768.0;
            self.sum_squares += v * v;
            self.peak = self.peak.max(v.abs() as f32);
            self.count += 1;
            if self.count >= self.window {
                out.push(self.flush());
            }
        }
        out
    }

    /// Emits the partial window, if any, and resets.
    pub fn flush(&mut self) -> (f32, f32) {
        let rms = if self.count == 0 {
            0.0
        } else {
            (self.sum_squares / self.count as f64).sqrt() as f32
        };
        let peak = self.peak;
        self.count = 0;
        self.sum_squares = 0.0;
        self.peak = 0.0;
        (rms.clamp(0.0, 1.0), peak.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_emit_rms_and_peak_and_silence_is_zero() {
        let mut meter = Meter::new(50);
        let silence = vec![0i16; 800];
        let levels = meter.push(&silence);
        assert_eq!(levels, vec![(0.0, 0.0)]);
        let loud: Vec<i16> = (0..1600)
            .map(|i| if i % 2 == 0 { 16_384 } else { -16_384 })
            .collect();
        let levels = meter.push(&loud);
        assert_eq!(levels.len(), 2);
        assert!((levels[0].0 - 0.5).abs() < 0.01, "{levels:?}");
        assert!((levels[0].1 - 0.5).abs() < 0.01);
        assert_eq!(meter.push(&[32_767; 10]).len(), 0);
        let (rms, peak) = meter.flush();
        assert!(rms > 0.99 && peak > 0.99);
    }
}
