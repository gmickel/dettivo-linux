//! `dettivo-qa pipeline import-merge` (ADR 0022): the jfk clip four times
//! with a second of silence between takes is cut by the real chunker at
//! settings that put every cut inside a gap and every chunk start inside
//! a take, each chunk runs through `dettivo-engine-whisper` in CLI mode,
//! the merger reconciles the overlaps, and the result is compared with
//! the golden: every take once, none duplicated across an overlap, and a
//! word error rate under the ceiling. The report lands under the evidence
//! directory.

use std::path::{Path, PathBuf};
use std::process::Command;

use dettivo_transcribe::merger::{self, ChunkResult};
use dettivo_transcribe::{AudioSource, Settings, WavSource, chunker};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scenarios;

/// The golden transcript, relative to the repository root.
pub const GOLDEN: &str = "crates/dettivo-qa/fixtures/import-merge/golden.txt";
/// Takes of the clip in the fixture.
pub const TAKES: usize = 4;
/// The word error rate the merged text must stay under.
pub const WER_CEILING: f64 = 0.10;

/// Chunk settings that cut inside every gap of the fixture (takes are
/// twelve seconds: eleven of speech, one of silence).
pub fn settings() -> Settings {
    Settings {
        chunk_seconds: 16,
        overlap_seconds: 2,
        safety_margin_seconds: 5,
        ..Settings::default()
    }
}

/// One chunk as it went through the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkRow {
    /// Zero-based index.
    pub index: u32,
    /// Start on the audio's clock.
    pub start_ms: u64,
    /// End on the audio's clock.
    pub end_ms: u64,
    /// The engine's text for the chunk alone.
    pub text: String,
    /// Segments the engine returned.
    pub segments: usize,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The rule, in words, fixed before the run.
    pub rule: String,
    /// The chunk settings used.
    pub chunk_seconds: u64,
    /// The overlap used.
    pub overlap_seconds: u64,
    /// The margin used.
    pub safety_margin_seconds: u64,
    /// The backend the engine ran on.
    pub backend: String,
    /// The chunks.
    pub chunks: Vec<ChunkRow>,
    /// The merged transcript.
    pub merged_text: String,
    /// Merged segments.
    pub merged_segments: usize,
    /// Takes found in the merged text (`fellow americans`).
    pub takes_found: usize,
    /// Word error rate against the golden.
    pub wer: f64,
    /// `pass`, `fail` or `skipped`.
    pub verdict: String,
    /// Why.
    pub verdict_reason: String,
}

fn rule() -> String {
    format!(
        "pass when the merged text holds every one of the {TAKES} takes exactly once (no take lost at a cut, none duplicated across an overlap) and its word error rate against {GOLDEN} stays under {WER_CEILING}; skipped when the model, the clip or the engine is unavailable"
    )
}

fn models_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.is_dir().then_some(dir)
}

/// Lower-case alphanumeric words.
pub fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word error rate: edit distance over words divided by the reference length.
pub fn wer(reference: &[String], hypothesis: &[String]) -> f64 {
    let (n, m) = (reference.len(), hypothesis.len());
    if n == 0 {
        return if m == 0 { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    for i in 1..=n {
        let mut row = vec![i; m + 1];
        for j in 1..=m {
            let cost = usize::from(reference[i - 1] != hypothesis[j - 1]);
            row[j] = (prev[j] + 1).min(row[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = row;
    }
    prev[m] as f64 / n as f64
}

/// The clip `TAKES` times with a second of silence after each take.
fn fixture(clip: &Path, out: &Path) -> Result<(), String> {
    let mut reader = hound::WavReader::open(clip).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let mut writer = hound::WavWriter::create(out, spec).map_err(|e| e.to_string())?;
    for _ in 0..TAKES {
        for s in &samples {
            writer.write_sample(*s).map_err(|e| e.to_string())?;
        }
        for _ in 0..spec.sample_rate {
            writer.write_sample(0i16).map_err(|e| e.to_string())?;
        }
    }
    writer.finalize().map_err(|e| e.to_string())
}

fn write_chunk(samples: &[i16], out: &Path) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(out, spec).map_err(|e| e.to_string())?;
    for s in samples {
        writer.write_sample(*s).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

fn engine(binary: &Path, model: &Path, wav: &Path, cpu: bool) -> Result<Value, String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--wav")
        .arg(wav)
        .arg("--model")
        .arg(model)
        .arg("--json");
    if cpu {
        cmd.arg("--cpu");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    if !out.status.success() {
        return Err(format!(
            "engine failed on {}: {}",
            wav.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("engine JSON: {e}"))
}

/// Runs the pipeline; `work` holds the fixture and the chunk files.
pub fn import_merge(repo: &Path, work: &Path, cpu: bool) -> Result<Report, String> {
    let s = settings();
    let mut report = Report {
        rule: rule(),
        chunk_seconds: s.chunk_seconds,
        overlap_seconds: s.overlap_seconds,
        safety_margin_seconds: s.safety_margin_seconds,
        backend: String::new(),
        chunks: Vec::new(),
        merged_text: String::new(),
        merged_segments: 0,
        takes_found: 0,
        wer: 1.0,
        verdict: "skipped".into(),
        verdict_reason: String::new(),
    };
    let golden =
        std::fs::read_to_string(repo.join(GOLDEN)).map_err(|e| format!("{GOLDEN}: {e}"))?;
    let Some(models) = models_dir() else {
        report.verdict_reason = "no models directory (scripts/models/fetch-test-model.sh)".into();
        return Ok(report);
    };
    let clip = models.join("fixtures/jfk.wav");
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    if !clip.is_file() || !model.is_file() {
        report.verdict_reason =
            "tiny.en or jfk.wav missing (scripts/models/fetch-test-model.sh)".into();
        return Ok(report);
    }
    let binary = match scenarios::binary(repo, "dettivo-engine-whisper") {
        Ok(b) => b,
        Err(e) => {
            report.verdict_reason = e;
            return Ok(report);
        }
    };
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let wav = work.join("fixture.wav");
    fixture(&clip, &wav)?;
    let mut source = WavSource::open(&wav)?;
    let plan = chunker::plan(&mut source, &s)?;
    let mut results = Vec::new();
    for chunk in &plan {
        let pcm = source.read(chunk.start, chunk.end)?;
        let path = work.join(format!("chunk_{}.wav", chunk.index));
        write_chunk(&pcm, &path)?;
        let value = engine(&binary, &model, &path, cpu)?;
        let segments: Vec<dettivo_transcribe::Segment> =
            serde_json::from_value(value["segments"].clone()).map_err(|e| e.to_string())?;
        report.backend = value["backend"].as_str().unwrap_or("").to_string();
        report.chunks.push(ChunkRow {
            index: chunk.index,
            start_ms: chunk.start_ms(),
            end_ms: chunk.end_ms(),
            text: value["text"].as_str().unwrap_or("").to_string(),
            segments: segments.len(),
        });
        results.push(ChunkResult {
            start_ms: chunk.start_ms(),
            end_ms: chunk.end_ms(),
            segments,
        });
    }
    let merged = merger::merge(&results);
    report.merged_segments = merged.len();
    report.merged_text = merger::text_of(&merged);
    let lower = report.merged_text.to_lowercase();
    report.takes_found = lower.matches("fellow americans").count();
    report.wer = wer(&words(&golden), &words(&report.merged_text));
    let duplicated = lower.matches("ask not").count() > TAKES;
    if report.takes_found == TAKES && !duplicated && report.wer < WER_CEILING {
        report.verdict = "pass".into();
        report.verdict_reason = format!(
            "{} chunks, {TAKES} takes once each, wer {:.3}",
            plan.len(),
            report.wer
        );
    } else {
        report.verdict = "fail".into();
        report.verdict_reason = format!(
            "{} takes found of {TAKES}{}, wer {:.3} (ceiling {WER_CEILING})",
            report.takes_found,
            if duplicated {
                ", a take duplicated across an overlap"
            } else {
                ""
            },
            report.wer
        );
    }
    Ok(report)
}

/// Human rendering.
pub fn human(report: &Report) -> String {
    let mut out = format!(
        "import-merge: {} ({})\n",
        report.verdict, report.verdict_reason
    );
    for c in &report.chunks {
        out.push_str(&format!(
            "  chunk {} {:>7} ms - {:>7} ms  {} segments  {}\n",
            c.index,
            c.start_ms,
            c.end_ms,
            c.segments,
            c.text.chars().take(60).collect::<String>()
        ));
    }
    if !report.merged_text.is_empty() {
        out.push_str(&format!(
            "  merged: {} segments, wer {:.3}, backend {}\n",
            report.merged_segments, report.wer, report.backend
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_error_rate_counts_edits_over_the_reference() {
        let r = words("And so, my fellow Americans!");
        assert_eq!(r, ["and", "so", "my", "fellow", "americans"]);
        assert_eq!(wer(&r, &r), 0.0);
        assert!((wer(&r, &words("and so my fellow")) - 0.2).abs() < 1e-9);
        assert!((wer(&r, &words("and so my fellow americans again")) - 0.2).abs() < 1e-9);
        assert_eq!(wer(&[], &[]), 0.0);
        let s = settings();
        assert!(s.chunk_seconds > s.overlap_seconds + s.safety_margin_seconds);
    }
}
