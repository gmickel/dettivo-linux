//! `dettivo-qa pipeline diarization` (ADR 0035): the two-speaker fixture
//! through `dettivo-engine-diarize` in CLI mode, scored against the
//! expected turns by diarization error rate (missed speech, false alarm
//! and speaker confusion over the reference speech, with the speaker
//! mapping that minimises it), and passing under the ceiling with exactly
//! two speakers. `--bench` records the realtime factor for NFR-4. The
//! report lands under the evidence directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::scenarios;

mod measurement;
pub use measurement::CpuComparison;
pub(crate) use measurement::EngineAnswer;

/// The fixture and its expected turns, relative to the repository root.
pub const FIXTURE: &str = "crates/dettivo-qa/fixtures/diarization/two-speakers.wav";
/// The expected turns beside the fixture.
pub const TURNS: &str = "crates/dettivo-qa/fixtures/diarization/two-speakers.turns.json";
/// The diarization error rate the fixture must stay under.
pub const DER_CEILING: f64 = 0.20;
/// The realtime factor (audio seconds per wall second) NFR-4 asks for.
pub const REALTIME_FLOOR: f64 = 4.0;
/// The step the timeline is scored at, in milliseconds.
const STEP_MS: u64 = 10;

/// One turn, expected or answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    /// The speaker.
    pub speaker: String,
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
}

/// The expected turns file.
#[derive(Debug, Clone, Deserialize)]
struct Expected {
    speakers: usize,
    duration_ms: u64,
    turns: Vec<Turn>,
}

/// The diarization error rate and its parts, as shares of the reference
/// speech.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRate {
    /// Reference speech the answer left unlabelled.
    pub missed: f64,
    /// Answered speech where the reference has none.
    pub false_alarm: f64,
    /// Speech labelled with the wrong speaker under the best mapping.
    pub confusion: f64,
    /// The sum.
    pub der: f64,
    /// The mapping from reference speaker to answered label that gave it.
    pub mapping: Vec<(String, String)>,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The rule, in words, fixed before the run.
    pub rule: String,
    /// The fixture.
    pub fixture: String,
    /// The model directory.
    pub model: String,
    /// The backend reported by the engine, absent when skipped.
    pub backend: Option<String>,
    /// Why automatic provider selection fell back to CPU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    /// The matched CPU run for a CUDA benchmark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_comparison: Option<CpuComparison>,
    /// Expected speakers.
    pub expected_speakers: usize,
    /// Speakers the engine found.
    pub found_speakers: usize,
    /// The turns the engine answered.
    pub turns: Vec<Turn>,
    /// The error rate.
    pub error_rate: Option<ErrorRate>,
    /// Audio length in milliseconds.
    pub audio_ms: u64,
    /// Wall time of the CLI run in milliseconds (the load included).
    pub elapsed_ms: u64,
    /// Audio seconds per wall second, when `--bench` asked for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realtime_factor: Option<f64>,
    /// `pass`, `fail` or `skipped`.
    pub verdict: String,
    /// Why.
    pub verdict_reason: String,
}

fn rule() -> String {
    format!(
        "pass when the engine finds exactly the expected speakers and the diarization error rate against {TURNS} (missed plus false alarm plus confusion over the reference speech, scored every {STEP_MS} ms under the best speaker mapping) stays under {DER_CEILING}; with --bench the realtime factor must reach {REALTIME_FLOOR}, and CUDA must preserve the forced-CPU DER and speaker count with at least 4x CPU throughput; skipped when the model set or the engine is unavailable"
    )
}

/// The model set on this machine, when downloaded.
fn model_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models/diarize/diarization-en");
    (dir.join("segmentation.onnx").is_file() && dir.join("embedding.onnx").is_file()).then_some(dir)
}

/// The speaker at `t` in `turns`: every turn covering it (overlaps keep
/// every speaker).
fn speakers_at(turns: &[Turn], t: u64) -> Vec<&str> {
    turns
        .iter()
        .filter(|turn| turn.start_ms <= t && t < turn.end_ms)
        .map(|turn| turn.speaker.as_str())
        .collect()
}

/// Scores `answer` against `reference` over `duration_ms` under every
/// one-to-one mapping of reference speakers to answered labels, keeping
/// the best.
pub fn error_rate(reference: &[Turn], answer: &[Turn], duration_ms: u64) -> ErrorRate {
    let mut ref_speakers: Vec<String> = Vec::new();
    for t in reference {
        if !ref_speakers.contains(&t.speaker) {
            ref_speakers.push(t.speaker.clone());
        }
    }
    let mut labels: Vec<String> = Vec::new();
    for t in answer {
        if !labels.contains(&t.speaker) {
            labels.push(t.speaker.clone());
        }
    }
    let mut best: Option<ErrorRate> = None;
    for mapping in mappings(&ref_speakers, &labels) {
        let scored = score(reference, answer, duration_ms, &mapping);
        if best.as_ref().is_none_or(|b| scored.der < b.der) {
            best = Some(scored);
        }
    }
    best.unwrap_or_else(|| score(reference, answer, duration_ms, &[]))
}

/// Every injective mapping of `speakers` onto `labels` (small counts).
fn mappings(speakers: &[String], labels: &[String]) -> Vec<Vec<(String, String)>> {
    fn go(
        speakers: &[String],
        labels: &[String],
        used: &mut Vec<bool>,
        current: &mut Vec<(String, String)>,
        out: &mut Vec<Vec<(String, String)>>,
    ) {
        if current.len() == speakers.len() {
            out.push(current.clone());
            return;
        }
        let speaker = &speakers[current.len()];
        let mut any = false;
        for (i, label) in labels.iter().enumerate() {
            if used[i] {
                continue;
            }
            any = true;
            used[i] = true;
            current.push((speaker.clone(), label.clone()));
            go(speakers, labels, used, current, out);
            current.pop();
            used[i] = false;
        }
        if !any {
            // More reference speakers than labels: the rest stay unmapped.
            out.push(current.clone());
        }
    }
    let mut out = Vec::new();
    go(
        speakers,
        labels,
        &mut vec![false; labels.len()],
        &mut Vec::new(),
        &mut out,
    );
    out
}

fn score(
    reference: &[Turn],
    answer: &[Turn],
    duration_ms: u64,
    mapping: &[(String, String)],
) -> ErrorRate {
    let (mut reference_ms, mut missed, mut false_alarm, mut confusion) = (0u64, 0u64, 0u64, 0u64);
    let mut t = 0;
    while t < duration_ms {
        let expected = speakers_at(reference, t);
        let got = speakers_at(answer, t);
        reference_ms += STEP_MS * expected.len() as u64;
        let mapped: Vec<&str> = expected
            .iter()
            .filter_map(|s| {
                mapping
                    .iter()
                    .find(|(r, _)| r == s)
                    .map(|(_, l)| l.as_str())
            })
            .collect();
        let matched = mapped.iter().filter(|l| got.contains(l)).count() as u64;
        let expected_n = expected.len() as u64;
        let got_n = got.len() as u64;
        if got_n < expected_n {
            missed += STEP_MS * (expected_n - got_n);
        }
        if got_n > expected_n {
            false_alarm += STEP_MS * (got_n - expected_n);
        }
        let compared = expected_n.min(got_n);
        confusion += STEP_MS * compared.saturating_sub(matched);
        t += STEP_MS;
    }
    let denominator = reference_ms.max(1) as f64;
    let (missed, false_alarm, confusion) = (
        missed as f64 / denominator,
        false_alarm as f64 / denominator,
        confusion as f64 / denominator,
    );
    ErrorRate {
        missed,
        false_alarm,
        confusion,
        der: missed + false_alarm + confusion,
        mapping: mapping.to_vec(),
    }
}

fn skipped(reason: String, model: String) -> Report {
    Report {
        rule: rule(),
        fixture: FIXTURE.into(),
        model,
        backend: None,
        fallback_reason: None,
        cpu_comparison: None,
        expected_speakers: 0,
        found_speakers: 0,
        turns: Vec::new(),
        error_rate: None,
        audio_ms: 0,
        elapsed_ms: 0,
        realtime_factor: None,
        verdict: "skipped".into(),
        verdict_reason: reason,
    }
}

/// Runs the pipeline; `bench` records the realtime factor and holds it to
/// the floor.
pub fn diarization(repo: &Path, bench: bool) -> Result<Report, String> {
    diarization_with_provider(repo, bench, "auto")
}

pub(crate) fn diarization_with_provider(
    repo: &Path,
    bench: bool,
    provider: &str,
) -> Result<Report, String> {
    let expected_path = repo.join(TURNS);
    let expected: Expected = std::fs::read_to_string(&expected_path)
        .map_err(|e| format!("{}: {e}", expected_path.display()))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))?;
    let Some(model) = model_dir() else {
        return Ok(skipped(
            "the diarization model set is not downloaded (scripts/models/fetch-diarization-model.sh)".into(),
            String::new(),
        ));
    };
    let binary = match scenarios::binary(repo, "dettivo-engine-diarize") {
        Ok(b) => b,
        Err(e) => return Ok(skipped(e, model.to_string_lossy().into_owned())),
    };
    let wav = repo.join(FIXTURE);
    let (answer, elapsed_ms) =
        measurement::run(&binary, &wav, &model, expected.speakers, provider)?;
    let turns = answer.turns;
    let mut labels: Vec<&str> = turns.iter().map(|t| t.speaker.as_str()).collect();
    labels.sort_unstable();
    labels.dedup();
    let rate = error_rate(&expected.turns, &turns, expected.duration_ms);
    let realtime_factor = bench.then(|| expected.duration_ms as f64 / elapsed_ms.max(1) as f64);
    let mut reasons = Vec::new();
    let cpu_comparison = if bench && answer.backend == "cuda" {
        let (cpu, cpu_elapsed_ms) =
            measurement::run(&binary, &wav, &model, expected.speakers, "cpu")?;
        let comparison = CpuComparison::new(&expected, &cpu.turns, cpu_elapsed_ms, elapsed_ms);
        reasons.extend(comparison.failures(rate.der, labels.len()));
        Some(comparison)
    } else {
        None
    };
    if labels.len() != expected.speakers {
        reasons.push(format!(
            "{} speakers found, {} expected",
            labels.len(),
            expected.speakers
        ));
    }
    if rate.der >= DER_CEILING {
        reasons.push(format!("DER {:.3} is not under {DER_CEILING}", rate.der));
    }
    if let Some(rtf) = realtime_factor
        && rtf < REALTIME_FLOOR
    {
        reasons.push(format!(
            "realtime factor {rtf:.1} is under {REALTIME_FLOOR}"
        ));
    }
    let mut verdict_reason = if reasons.is_empty() {
        format!(
            "{} speakers, DER {:.3} (missed {:.3}, false alarm {:.3}, confusion {:.3}) on {}{}",
            labels.len(),
            rate.der,
            rate.missed,
            rate.false_alarm,
            rate.confusion,
            answer.backend,
            realtime_factor
                .map(|r| format!(", {r:.1}x realtime"))
                .unwrap_or_default()
        )
    } else {
        reasons.join("; ")
    };
    if let Some(cpu) = &cpu_comparison {
        verdict_reason.push_str(&format!(
            "; CPU DER {:.6}, CUDA DER {:.6}; CPU {:.3}x realtime, CUDA {:.3}x realtime, {:.3}x speedup",
            cpu.error_rate.der, rate.der, cpu.realtime_factor,
            realtime_factor.unwrap_or_default(), cpu.speedup,
        ));
    }
    Ok(Report {
        rule: rule(),
        fixture: FIXTURE.into(),
        model: model.to_string_lossy().into_owned(),
        backend: Some(answer.backend),
        fallback_reason: answer.fallback_reason,
        cpu_comparison,
        expected_speakers: expected.speakers,
        found_speakers: labels.len(),
        turns,
        error_rate: Some(rate),
        audio_ms: expected.duration_ms,
        elapsed_ms,
        realtime_factor,
        verdict: if reasons.is_empty() { "pass" } else { "fail" }.into(),
        verdict_reason,
    })
}

/// The `pipeline diarization` subcommand: runs it, files the report under
/// `evidence`, prints it; the exit code is 1 on `fail`, 0 on `pass` or
/// `skipped`.
pub fn run_cli(json_out: bool, repo: &Path, bench: bool, evidence: &Path) -> u8 {
    let dir = match crate::evidence::Evidence::run_for_process(evidence).and_then(|run| {
        let dir = run.run_dir.join("diarization");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("pipeline: evidence: {e}");
            return 1;
        }
    };
    let report = match diarization(repo, bench) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pipeline: {e}");
            return 1;
        }
    };
    let json = serde_json::to_string_pretty(&report).unwrap_or_default() + "\n";
    let path = dir.join("diarization-report.json");
    if let Err(e) = std::fs::write(&path, &json) {
        eprintln!("pipeline: {}: {e}", path.display());
        return 1;
    }
    if json_out {
        print!("{json}");
    } else {
        println!(
            "diarization: {} ({})",
            report.verdict, report.verdict_reason
        );
        for t in &report.turns {
            println!("  {:>7} {:>7}  {}", t.start_ms, t.end_ms, t.speaker);
        }
        println!("  report: {}", path.display());
    }
    u8::from(report.verdict == "fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(speaker: &str, start: u64, end: u64) -> Turn {
        Turn {
            speaker: speaker.into(),
            start_ms: start,
            end_ms: end,
        }
    }

    #[test]
    fn the_error_rate_finds_the_mapping_and_counts_every_part() {
        let reference = vec![turn("A", 0, 1000), turn("B", 1000, 2000)];
        let perfect = vec![turn("S1", 0, 1000), turn("S0", 1000, 2000)];
        let rate = error_rate(&reference, &perfect, 2000);
        assert_eq!(rate.der, 0.0);
        assert!(rate.mapping.contains(&("A".to_string(), "S1".to_string())));
        // Half of B missed, a false alarm after the end, and A's second
        // half confused with the other label.
        let flawed = vec![
            turn("S1", 0, 500),
            turn("S0", 500, 1000),
            turn("S0", 1000, 1500),
            turn("S1", 2000, 2200),
        ];
        let rate = error_rate(&reference, &flawed, 2200);
        assert!((rate.missed - 0.25).abs() < 0.01, "{rate:?}");
        assert!((rate.false_alarm - 0.10).abs() < 0.01, "{rate:?}");
        assert!((rate.confusion - 0.25).abs() < 0.01, "{rate:?}");
        assert!((rate.der - 0.60).abs() < 0.01, "{rate:?}");
        let one = vec![turn("S0", 0, 2000)];
        let rate = error_rate(&reference, &one, 2000);
        assert!((rate.confusion - 0.5).abs() < 0.01, "{rate:?}");
        assert_eq!(
            mappings(&["A".into(), "B".into()], &["x".into(), "y".into()]).len(),
            2
        );
    }
}
