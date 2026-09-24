//! The `meetings` block of a checked-in benchmark report (ADR 0039):
//! what `dettivo-qa pack meetings --record` files beside the bench
//! steps, the meeting and diarization realtime factors beside their
//! targets with the engine, model, backend, GPU proof and harness load
//! that qualify them. The README renders it as rows under the host and
//! tier the pack ran on.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::report::Comparison;

/// The block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingsBlock {
    /// Unix time the pack run started.
    pub recorded_unix: u64,
    /// The date, `YYYY-MM-DD`.
    pub date: String,
    /// The commit the pack ran from.
    pub git_sha: String,
    /// The pack's run directory.
    pub pack_run_dir: String,
    /// Whether every must-pass step of that run passed; a block from a
    /// run with a failed step says so beside its figures.
    pub pack_passed: bool,
    /// `DETTIVO_FORCE_CPU=1` (`--cpu`).
    pub forced_cpu: bool,
    /// `whisper`.
    pub engine: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// The audio imported.
    pub audio_ms: u64,
    /// The finalisation's wall time.
    pub wall_ms: u64,
    /// The fixture's SHA-256.
    pub fixture_sha256: String,
    /// The meeting realtime factor against NFR-4 or NFR-5.
    pub meeting: Comparison,
    /// The diarization realtime factor against NFR-4.
    pub diarization: Comparison,
    /// What `gpu_workload_proof` concluded (`pass: ...`, `skip: ...`).
    pub gpu_workload_proof: String,
    /// The harness's CPU share during the import.
    pub harness_cpu_pct: Option<f64>,
    /// The warning when the harness share passed the line.
    pub harness_warning: Option<String>,
    /// The token scenario's coverage, as it wrote it.
    pub token_coverage: Option<Value>,
}

fn figure(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.2} x realtime"))
        .unwrap_or_else(|| "-".into())
}

fn verdict(met: Option<bool>) -> &'static str {
    match met {
        Some(true) => "met",
        Some(false) => "not met",
        None => "not compared",
    }
}

/// The rows the block adds to the README table, in order; `rows` renders
/// one each and the evidence map resolves `bench` routes against them.
pub const ROWS: &[&str] = &[
    "meeting_throughput",
    "diarization_throughput",
    "gpu_workload_proof",
    "meeting_token_coverage",
];

impl MeetingsBlock {
    /// The README rows, in the steps table's shape.
    pub fn rows(&self) -> Vec<String> {
        let mut out = vec![
            format!(
                "| `meeting_throughput` · {} {} ({}) | {} | {} | {} | >= {} | {} |",
                self.engine,
                self.model,
                self.backend,
                self.meeting.nfr,
                self.meeting.metric,
                figure(self.meeting.value),
                figure(Some(self.meeting.target)),
                verdict(self.meeting.met)
            ),
            format!(
                "| `diarization_throughput` | {} | {} | {} | >= {} | {} |",
                self.diarization.nfr,
                self.diarization.metric,
                figure(self.diarization.value),
                figure(Some(self.diarization.target)),
                verdict(self.diarization.met)
            ),
            format!(
                "| `gpu_workload_proof` | - | - | - | - | {} |",
                self.gpu_workload_proof.replace('|', "\\|")
            ),
        ];
        if let Some(c) = &self.token_coverage {
            out.push(format!(
                "| `meeting_token_coverage` | - | token_coverage | {} of {} tokens | every token on its source | {} |",
                c["found"].as_u64().unwrap_or(0),
                c["expected"].as_u64().unwrap_or(0),
                if c["missing"].as_array().is_some_and(Vec::is_empty) {
                    "met"
                } else {
                    "not met"
                }
            ));
        }
        out
    }

    /// The note beneath the rows.
    pub fn note(&self) -> String {
        format!(
            "meetings: recorded {} from commit `{}` by `dettivo-qa pack meetings{}`, the pack {} ({} ms of audio in {} ms{})",
            self.date,
            &self.git_sha[..self.git_sha.len().min(12)],
            if self.forced_cpu { " --cpu" } else { "" },
            if self.pack_passed {
                "passed"
            } else {
                "FAILED on another step"
            },
            self.audio_ms,
            self.wall_ms,
            self.harness_warning
                .as_deref()
                .map(|w| format!("; {w}"))
                .unwrap_or_default()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nfr;

    #[test]
    fn the_block_renders_its_rows_and_note() {
        let block = MeetingsBlock {
            recorded_unix: 1,
            date: "2026-09-05".into(),
            git_sha: "abcdef123456789".into(),
            pack_run_dir: "qa-evidence/run".into(),
            pack_passed: false,
            forced_cpu: true,
            engine: "whisper".into(),
            model: "base.en".into(),
            backend: "cpu".into(),
            audio_ms: 300_000,
            wall_ms: 30_000,
            fixture_sha256: "0".repeat(64),
            meeting: Comparison::of(&nfr::NFR5_STT_REALTIME_CPU, Some(10.0)),
            diarization: Comparison::of(&nfr::NFR4_DIARIZATION_REALTIME_GPU, Some(3.0)),
            gpu_workload_proof: "skip: the run was on the cpu tier (--cpu)".into(),
            harness_cpu_pct: Some(1.5),
            harness_warning: None,
            token_coverage: Some(serde_json::json!({"found": 8, "expected": 8, "missing": []})),
        };
        let rows = block.rows();
        assert_eq!(
            rows[0],
            "| `meeting_throughput` · whisper base.en (cpu) | NFR-5 | stt_realtime_factor | 10.00 x realtime | >= 5.00 x realtime | met |"
        );
        assert!(rows[1].contains("| NFR-4 | diarization_realtime_factor | 3.00 x realtime | >= 4.00 x realtime | not met |"), "{}", rows[1]);
        assert!(rows[3].contains("8 of 8 tokens"), "{}", rows[3]);
        assert!(
            block.note().contains("--cpu") && block.note().contains("FAILED"),
            "{}",
            block.note()
        );
    }
}
