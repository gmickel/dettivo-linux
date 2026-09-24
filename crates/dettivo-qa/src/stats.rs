//! The arithmetic the pack report shares with the scenarios that feed it:
//! nearest-rank percentiles over millisecond samples and the reliability
//! ratio over attempted rows.

/// The nearest-rank percentile of `samples` (`q` in `0.0..=1.0`): the
/// smallest value with at least `q` of the samples at or below it, so p50
/// of ten samples is the fifth smallest and p95 the tenth. `None` for no
/// samples.
pub fn percentile(samples: &[u64], q: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = (q * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

/// Passes over attempted rows as a ratio in `0.0..=1.0`; `None` when
/// nothing was attempted.
pub fn reliability(passed: usize, attempted: usize) -> Option<f64> {
    (attempted > 0).then(|| passed as f64 / attempted as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_use_nearest_rank() {
        let ten: Vec<u64> = (1..=10).map(|n| n * 100).collect();
        assert_eq!(percentile(&ten, 0.5), Some(500));
        assert_eq!(percentile(&ten, 0.95), Some(1000));
        assert_eq!(percentile(&[7], 0.5), Some(7));
        assert_eq!(percentile(&[9, 3, 5], 0.5), Some(5));
        assert_eq!(percentile(&[], 0.5), None);
    }

    #[test]
    fn reliability_is_passes_over_attempted() {
        assert_eq!(reliability(49, 50), Some(0.98));
        assert_eq!(reliability(0, 0), None);
    }
}
