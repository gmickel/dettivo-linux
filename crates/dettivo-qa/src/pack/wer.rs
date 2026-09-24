//! The Whisper word-error-rate step: the engine binary transcribes the
//! speech fixture with the local tiny.en model, the words are compared
//! with the reference text by edit distance, and `wer.json` records the
//! rate against the threshold with the backend that ran (CPU in CI,
//! Vulkan on a machine whose engine was built with it).

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::scenarios::binary;

/// The rate the fixture must stay under (the engine's own test pins the
/// same figure; tiny.en on CPU scores 0).
pub const MAX_WER: f64 = 0.15;

/// `wer.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WerResult {
    /// Word error rate over the reference words.
    pub rate: f64,
    /// The threshold the rate is held against.
    pub threshold: f64,
    /// The backend the engine reported.
    pub backend: String,
    /// The model.
    pub model: String,
    /// Reference words.
    pub reference_words: usize,
    /// Hypothesis words.
    pub hypothesis_words: usize,
    /// The fixture's length as the engine reports it.
    pub audio_ms: u64,
    /// Wall time of the engine call, model load included.
    pub elapsed_ms: u64,
    /// Whether the CPU was forced.
    pub forced_cpu: bool,
}

/// Why the step did not produce a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The model, the fixture or the engine is missing.
    Skip(String),
    /// The engine failed or the rate is over the threshold.
    Fail(String),
}

/// The words of a text for the rate: lower case, split on anything that is
/// not a letter or a digit.
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Word error rate: edit distance over reference words.
pub fn wer(reference: &[String], hypothesis: &[String]) -> f64 {
    let (n, m) = (reference.len(), hypothesis.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(reference[i - 1] != hypothesis[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
        }
    }
    d[n][m] as f64 / n.max(1) as f64
}

/// Runs the fixture through the engine and writes `wer.json` into
/// `evidence_dir`. `force_cpu` passes `--cpu` (CI has no GPU; the release
/// gate on a Vulkan machine leaves it off).
pub fn run(
    repo_root: &Path,
    models_dir: Option<&Path>,
    force_cpu: bool,
    evidence_dir: &Path,
) -> Result<WerResult, Refusal> {
    let engine = binary(repo_root, "dettivo-engine-whisper").map_err(Refusal::Skip)?;
    let models = models_dir.ok_or_else(|| {
        Refusal::Skip("no model directory (scripts/models/fetch-test-model.sh)".into())
    })?;
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let wav = models.join("fixtures/jfk.wav");
    let text = models.join("fixtures/jfk.txt");
    for needed in [&model, &wav, &text] {
        if !needed.is_file() {
            return Err(Refusal::Skip(format!(
                "{} is missing (scripts/models/fetch-test-model.sh)",
                needed.display()
            )));
        }
    }
    let reference =
        words(&std::fs::read_to_string(&text).map_err(|e| Refusal::Fail(e.to_string()))?);
    let mut cmd = Command::new(&engine);
    cmd.arg("--wav")
        .arg(&wav)
        .arg("--model")
        .arg(&model)
        .arg("--json");
    if force_cpu {
        cmd.arg("--cpu");
    }
    let started = Instant::now();
    let out = cmd
        .output()
        .map_err(|e| Refusal::Fail(format!("{}: {e}", engine.display())))?;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let log = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::write(evidence_dir.join("engine.log"), &log);
    if !out.status.success() {
        return Err(Refusal::Fail(format!(
            "the engine exited {}: {}",
            out.status.code().unwrap_or(-1),
            log.lines().last().unwrap_or("")
        )));
    }
    let answer: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| Refusal::Fail(format!("engine output: {e}")))?;
    let hypothesis = words(answer["text"].as_str().unwrap_or(""));
    let result = WerResult {
        rate: wer(&reference, &hypothesis),
        threshold: MAX_WER,
        backend: answer["backend"].as_str().unwrap_or("unknown").to_string(),
        model: "tiny.en".into(),
        reference_words: reference.len(),
        hypothesis_words: hypothesis.len(),
        audio_ms: answer["duration_ms"].as_u64().unwrap_or(0),
        elapsed_ms,
        forced_cpu: force_cpu,
    };
    let json = serde_json::to_string_pretty(&result).map_err(|e| Refusal::Fail(e.to_string()))?;
    std::fs::write(evidence_dir.join("wer.json"), json)
        .map_err(|e| Refusal::Fail(e.to_string()))?;
    if result.rate >= MAX_WER {
        return Err(Refusal::Fail(format!(
            "WER {:.3} is not under {MAX_WER} on {}",
            result.rate, result.backend
        )));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wer_is_edit_distance_over_reference_words() {
        let reference = words("Ask not what your country can do for you");
        assert_eq!(wer(&reference, &reference), 0.0);
        let hypothesis = words("ask not what your county can do");
        let expected = 3.0 / 9.0;
        assert!((wer(&reference, &hypothesis) - expected).abs() < 1e-9);
        assert_eq!(wer(&[], &[]), 0.0);
    }
}
