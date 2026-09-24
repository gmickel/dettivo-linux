use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{ErrorRate, Expected, Turn, error_rate};

#[derive(Deserialize)]
pub(crate) struct EngineAnswer {
    pub turns: Vec<Turn>,
    pub backend: String,
    pub fallback_reason: Option<String>,
}

impl EngineAnswer {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        let answer: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("engine JSON: {e}"))?;
        if !matches!(answer.backend.as_str(), "cpu" | "cuda") {
            return Err(format!("engine JSON: invalid backend {:?}", answer.backend));
        }
        Ok(answer)
    }
}

/// A forced-CPU run of the same binary, model and audio as the CUDA run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuComparison {
    /// CPU wall time including model load.
    pub elapsed_ms: u64,
    /// CPU audio seconds per wall second.
    pub realtime_factor: f64,
    /// CPU diarization error rate against the same reference.
    pub error_rate: ErrorRate,
    /// Speakers in the CPU answer.
    pub found_speakers: usize,
    /// CUDA realtime factor divided by CPU realtime factor.
    pub speedup: f64,
}

impl CpuComparison {
    pub(super) fn new(expected: &Expected, turns: &[Turn], cpu_ms: u64, cuda_ms: u64) -> Self {
        let speakers: std::collections::BTreeSet<_> = turns.iter().map(|t| &t.speaker).collect();
        Self {
            elapsed_ms: cpu_ms,
            realtime_factor: expected.duration_ms as f64 / cpu_ms.max(1) as f64,
            error_rate: error_rate(&expected.turns, turns, expected.duration_ms),
            found_speakers: speakers.len(),
            speedup: cpu_ms.max(1) as f64 / cuda_ms.max(1) as f64,
        }
    }

    pub(super) fn failures(&self, cuda_der: f64, cuda_speakers: usize) -> Vec<String> {
        let mut reasons = Vec::new();
        if self.error_rate.der != cuda_der {
            reasons.push(format!(
                "CPU DER {:.6} differs from CUDA DER {cuda_der:.6}",
                self.error_rate.der
            ));
        }
        if self.found_speakers != cuda_speakers {
            reasons.push(format!(
                "CPU found {} speakers, CUDA found {cuda_speakers}",
                self.found_speakers
            ));
        }
        if self.speedup < 4.0 {
            reasons.push(format!("CUDA speedup {:.3}x over CPU is under 4x (CPU {:.3}x realtime, CUDA {:.3}x realtime)", self.speedup, self.realtime_factor, self.realtime_factor * self.speedup));
        }
        reasons
    }
}

pub(super) fn run(
    binary: &Path,
    wav: &Path,
    model: &Path,
    speakers: usize,
    provider: &str,
) -> Result<(EngineAnswer, u64), String> {
    let started = Instant::now();
    let output = Command::new(binary)
        .arg("--wav")
        .arg(wav)
        .arg("--model")
        .arg(model)
        .args([
            "--speakers",
            &speakers.to_string(),
            "--provider",
            provider,
            "--json",
        ])
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if !output.status.success() {
        return Err(format!(
            "dettivo-engine-diarize ({provider}): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let answer = EngineAnswer::parse(&output.stdout)?;
    if provider != "auto" && answer.backend != provider {
        return Err(format!(
            "engine requested {provider}, reported {}",
            answer.backend
        ));
    }
    Ok((answer, elapsed_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_comparison_requires_identical_der_speakers_and_fourfold_speedup() {
        let comparison = CpuComparison {
            elapsed_ms: 4000,
            realtime_factor: 5.0,
            error_rate: ErrorRate {
                missed: 0.02,
                false_alarm: 0.0,
                confusion: 0.0,
                der: 0.02,
                mapping: vec![],
            },
            found_speakers: 2,
            speedup: 4.0,
        };
        assert!(comparison.failures(0.02, 2).is_empty());
        for der in [0.01, 0.03] {
            let failures = comparison.failures(der, 2).join("; ");
            assert!(
                failures.contains("CPU DER 0.020000")
                    && failures.contains(&format!("CUDA DER {der:.6}")),
                "{failures}"
            );
        }
        assert!(!comparison.failures(0.02, 1).is_empty());
        let slow = CpuComparison {
            speedup: 3.99,
            ..comparison
        };
        assert!(!slow.failures(0.02, 2).is_empty());
    }

    #[test]
    fn backend_must_be_reported_and_known() {
        for json in [r#"{"turns":[]}"#, r#"{"turns":[],"backend":"unknown"}"#] {
            assert!(EngineAnswer::parse(json.as_bytes()).is_err());
        }
        assert_eq!(
            EngineAnswer::parse(br#"{"turns":[],"backend":"cuda"}"#)
                .unwrap()
                .backend,
            "cuda"
        );
    }
}
