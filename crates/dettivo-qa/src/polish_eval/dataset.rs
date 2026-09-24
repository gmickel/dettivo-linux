//! The held-out sets in the macOS canonical JSONL shape
//! (`scripts/polish_finetune/*.jsonl` on macOS): one sample per line with
//! the transcript, the gold rewrite, the policy inputs and the fragments
//! the gates check. The spec's names for three fields (`expected`,
//! `bundle_id`, `protected_tokens`) read as aliases, so a set written
//! either way loads; fields the scorer does not read (`provenance`,
//! `review`, `failure_types`) are skipped.

use std::path::Path;

use serde::{Deserialize, Serialize};

fn heldout() -> String {
    "heldout".into()
}

fn english() -> String {
    "en".into()
}

/// One row of a set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// The row id; the only sample field a report carries.
    pub id: String,
    /// `train`, `valid` or `heldout`.
    #[serde(default = "heldout")]
    pub split: String,
    /// The transcript as the engine returned it.
    pub input: String,
    /// The rewrite a reviewer accepted; a row without one is scored on
    /// the gates alone.
    #[serde(default, alias = "expected")]
    pub gold: Option<String>,
    /// The preset the row runs under.
    pub preset: String,
    /// The style, when the row names one.
    #[serde(default)]
    pub style: Option<String>,
    /// The language (`en`, `de`); a non-English row's required fragments
    /// double as its language anchors.
    #[serde(default = "english")]
    pub language: String,
    /// The app the policy resolves for, when the row names one.
    #[serde(default, alias = "bundle_id")]
    pub app_bundle_id: Option<String>,
    /// Text the rewrite must keep verbatim (paths, identifiers, numbers).
    #[serde(default, alias = "protected_tokens")]
    pub protected_spans: Vec<String>,
    /// Text the rewrite must keep.
    #[serde(default)]
    pub required_fragments: Vec<String>,
    /// Text the rewrite must not contain.
    #[serde(default)]
    pub forbidden_fragments: Vec<String>,
    /// Structure the rewrite must keep (list markers, line breaks).
    #[serde(default)]
    pub structure_fragments: Vec<String>,
}

/// Reads a set, keeping the rows of `split` (`all` keeps every row). A
/// line that is not a sample fails naming the line; an empty selection
/// fails naming the split.
pub fn load(path: &Path, split: &str) -> Result<Vec<Sample>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let sample: Sample = serde_json::from_str(line)
            .map_err(|e| format!("{}:{}: not a sample row ({e})", path.display(), i + 1))?;
        if split == "all" || sample.split == split {
            rows.push(sample);
        }
    }
    if rows.is_empty() {
        return Err(format!("{}: no rows for split {split:?}", path.display()));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_macos_shape_and_the_spec_aliases_both_load() {
        let dir = tempfile::Builder::new()
            .prefix("dtv-set")
            .tempdir_in("/tmp")
            .unwrap();
        let p = dir.path().join("set.jsonl");
        std::fs::write(
            &p,
            concat!(
                r#"{"id":"a","split":"heldout","input":"x","gold":"X","preset":"generic","style":"neutral","language":"de","app_bundle_id":"org.x","failure_types":["f"],"protected_spans":["p"],"required_fragments":["r"],"forbidden_fragments":["f"],"structure_fragments":[],"no_touch_spans":[],"provenance":{"origin":"o"},"review":{"author":"a"}}"#,
                "\n\n",
                r#"{"id":"b","input":"y","expected":"Y","preset":"email","bundle_id":"org.y","protected_tokens":["q"]}"#,
                "\n",
                r#"{"id":"c","split":"train","input":"z","preset":"email"}"#,
                "\n"
            ),
        )
        .unwrap();
        let rows = load(&p, "heldout").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].gold.as_deref(), Some("X"));
        assert_eq!(rows[0].language, "de");
        assert_eq!(rows[0].app_bundle_id.as_deref(), Some("org.x"));
        assert_eq!(rows[1].gold.as_deref(), Some("Y"));
        assert_eq!(rows[1].app_bundle_id.as_deref(), Some("org.y"));
        assert_eq!(rows[1].protected_spans, ["q"]);
        assert_eq!(rows[1].language, "en");
        assert_eq!(load(&p, "all").unwrap().len(), 3);
        assert!(load(&p, "valid").unwrap_err().contains("no rows for split"));
        std::fs::write(&p, "{\"id\":\"a\"}\n").unwrap();
        assert!(
            load(&p, "heldout")
                .unwrap_err()
                .contains(":1: not a sample row")
        );
    }
}
