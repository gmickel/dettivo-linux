//! What the token scenario judges with (fn-36, ADR 0039): the fixture's
//! token list, the coverage it writes as `token-coverage.json` (also read
//! by the meetings pack's measurements), the `md` export pulled through a
//! download transfer, and the judgement over the row and the export.

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Context;
use super::daemon::DaemonHandle;
use crate::pack::wer::words;

/// The fixture, relative to the repository root.
pub const FIXTURE_DIR: &str = "crates/dettivo-qa/fixtures/meetings";
/// The speaker names given after the pass.
pub const NAMES: [(&str, &str); 2] = [("microphone", "Alice"), ("system", "Ben")];

/// One token of the fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    /// The word, lower case.
    pub token: String,
    /// `microphone` or `system`.
    pub source: String,
    /// Whether the transcript carried it on that source.
    #[serde(default)]
    pub found: bool,
}

/// `alpha.tokens.json`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TokensFile {
    pub(crate) duration_ms: u64,
    pub(crate) tokens: Vec<Token>,
}

/// `token-coverage.json`, also read by the pack's measurements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TokenCoverage {
    /// `rig` (PipeWire null sinks) or `mock` (the fixture variables).
    pub capture: String,
    /// Tokens expected.
    pub expected: usize,
    /// Tokens found on their source.
    pub found: usize,
    /// `token (source)` for each token missing.
    pub missing: Vec<String>,
    /// Every token with its verdict.
    pub tokens: Vec<Token>,
    /// The speakers on the row after the pass and the renames.
    pub speakers: Vec<String>,
    /// Both names appear before a transcript line of the md export.
    pub md_names_found: bool,
    /// The diarization status the row settled at.
    pub diarization_status: String,
    /// The meeting id.
    pub meeting_id: String,
}

pub(crate) fn export_md(daemon: &DaemonHandle, id: &str) -> Result<String, String> {
    let begun = daemon.call(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    )?;
    let transfer = begun["transfer_id"].as_str().unwrap_or("").to_string();
    daemon.call(
        "transcripts.export",
        json!({"format": "md", "ref": {"id": id, "kind": "meeting"}, "transfer_id": transfer}),
    )?;
    let mut bytes = Vec::new();
    let mut seq = 1u64;
    loop {
        let chunk = daemon.call(
            "transfer.pull",
            json!({"transfer_id": transfer, "seq": seq}),
        )?;
        bytes.extend(
            base64::engine::general_purpose::STANDARD
                .decode(chunk["data_b64"].as_str().unwrap_or(""))
                .map_err(|e| format!("chunk {seq}: {e}"))?,
        );
        if chunk["eof"] == true {
            break;
        }
        seq += 1;
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The words on each source of the row.
fn words_by_source(got: &Value) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for seg in got["segments"].as_array().into_iter().flatten() {
        let source = seg["source_type"].as_str().unwrap_or("").to_string();
        let text = seg["text"].as_str().unwrap_or("");
        out.entry(source).or_default().extend(words(text));
    }
    out
}

pub(crate) fn judge(
    id: &str,
    tokens: &TokensFile,
    got: &Value,
    md: &str,
    capture: &str,
    speakers: Vec<String>,
    diarization_status: String,
) -> (TokenCoverage, Result<(), String>) {
    let by_source = words_by_source(got);
    let mut checked = tokens.tokens.clone();
    let mut missing = Vec::new();
    for t in &mut checked {
        t.found = by_source
            .get(&t.source)
            .is_some_and(|w| w.iter().any(|x| x == &t.token));
        if !t.found {
            missing.push(format!("{} ({})", t.token, t.source));
        }
    }
    let md_names_found = NAMES
        .iter()
        .all(|(_, name)| md.lines().any(|l| l.contains(&format!("] {name}: "))));
    let coverage = TokenCoverage {
        capture: capture.into(),
        expected: checked.len(),
        found: checked.iter().filter(|t| t.found).count(),
        missing: missing.clone(),
        tokens: checked,
        speakers,
        md_names_found,
        diarization_status: diarization_status.clone(),
        meeting_id: id.into(),
    };
    let mut problems = Vec::new();
    for m in &missing {
        problems.push(format!(
            "token {m} is missing from the finalised transcript"
        ));
    }
    if diarization_status != "ready" {
        problems.push(format!(
            "the speaker pass ended {diarization_status}: {}",
            got["diarization"]
        ));
    }
    if coverage.speakers.len() != 2 {
        problems.push(format!(
            "{} speakers on the row after the pass, 2 expected ({:?})",
            coverage.speakers.len(),
            coverage.speakers
        ));
    }
    if !md_names_found {
        problems.push("the md export does not name both Alice and Ben before a line".into());
    }
    let verdict = if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}; transcript: {:?}",
            problems.join("; "),
            got["transcript"].as_str().unwrap_or("")
        ))
    };
    (coverage, verdict)
}

pub(crate) fn write_evidence(
    ctx: &mut Context<'_>,
    got: &Value,
    md: &str,
    coverage: &TokenCoverage,
) -> Result<(), String> {
    let dir: PathBuf = ctx.evidence_dir.to_path_buf();
    std::fs::write(
        dir.join("token-coverage.json"),
        serde_json::to_string_pretty(coverage).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("transcript.json"),
        serde_json::to_string_pretty(&got["segments"]).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meeting.md"), md).map_err(|e| e.to_string())?;
    for name in ["token-coverage.json", "transcript.json", "meeting.md"] {
        ctx.evidence.push(name.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_judgement_names_the_missing_token_and_its_source() {
        let tokens = TokensFile {
            duration_ms: 1000,
            tokens: vec![
                Token {
                    token: "alice".into(),
                    source: "microphone".into(),
                    found: false,
                },
                Token {
                    token: "ship".into(),
                    source: "system".into(),
                    found: false,
                },
            ],
        };
        let got = json!({
            "transcript": "Alice here. We ship.",
            "segments": [
                {"source_type": "microphone", "text": "Alice here."},
                {"source_type": "system", "text": "We ship."}
            ],
            "diarization": {"status": "ready"}
        });
        let md = "- [00:00:00.000] Alice: Alice here.\n- [00:00:05.000] Ben: We ship.\n";
        let speakers = vec!["you -> Alice".into(), "speaker_00 -> Ben".into()];
        let (c, v) = judge(
            "m",
            &tokens,
            &got,
            md,
            "mock",
            speakers.clone(),
            "ready".into(),
        );
        assert!(v.is_ok(), "{v:?}");
        assert_eq!((c.found, c.expected), (2, 2));
        let swapped = json!({
            "transcript": "Alice here. We ship.",
            "segments": [
                {"source_type": "system", "text": "Alice here."},
                {"source_type": "microphone", "text": "We ship."}
            ]
        });
        let (c, v) = judge("m", &tokens, &swapped, md, "rig", speakers, "ready".into());
        let err = v.unwrap_err();
        assert!(err.contains("token alice (microphone) is missing"), "{err}");
        assert!(err.contains("token ship (system) is missing"), "{err}");
        assert_eq!(c.missing.len(), 2);
        let (_, v) = judge(
            "m",
            &tokens,
            &got,
            "no names",
            "mock",
            Vec::new(),
            "failed".into(),
        );
        let err = v.unwrap_err();
        assert!(
            err.contains("0 speakers")
                && err.contains("ended failed")
                && err.contains("Alice and Ben"),
            "{err}"
        );
    }

    #[test]
    fn the_checked_in_tokens_file_reads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text =
            std::fs::read_to_string(root.join(FIXTURE_DIR).join("alpha.tokens.json")).unwrap();
        let file: TokensFile = serde_json::from_str(&text).unwrap();
        assert_eq!(file.tokens.len(), 8);
        assert!(file.duration_ms > 9_000);
        assert!(root.join(FIXTURE_DIR).join("alpha.mic.wav").is_file());
        assert!(root.join(FIXTURE_DIR).join("alpha.system.wav").is_file());
    }
}
