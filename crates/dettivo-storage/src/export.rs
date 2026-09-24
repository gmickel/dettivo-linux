//! Export renderers: JSON, Markdown and plain text over a list of items
//! (goldens under `tests/goldens/`), and the archive that bundles the
//! items with their audio and restores into another profile with the same
//! ids.

use std::path::Path;

use serde_json::json;

use crate::item::{DictationItem, ItemStatus};
use crate::retention::{Artifacts, Retention};
use crate::{Store, StoreError, zip};

/// A dictation export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// JSON, every field.
    Json,
    /// Markdown, one section per item.
    Md,
    /// Plain text.
    Txt,
    /// A ZIP archive with `items.json` and the retained audio.
    Zip,
}

impl Format {
    /// Parses the wire or CLI spelling (`md` and `markdown` both work).
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "json" => Some(Self::Json),
            "md" | "markdown" => Some(Self::Md),
            "txt" | "text" => Some(Self::Txt),
            "zip" => Some(Self::Zip),
            _ => None,
        }
    }

    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Md => "md",
            Self::Txt => "txt",
            Self::Zip => "zip",
        }
    }

    /// The MIME type of the rendered payload.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Md => "text/markdown",
            Self::Txt => "text/plain",
            Self::Zip => "application/zip",
        }
    }

    /// The suggested file name for an export of `stem`.
    pub fn filename(self, stem: &str) -> String {
        format!("{stem}.{}", self.as_str())
    }
}

/// The name of the item list inside an archive.
pub const ARCHIVE_INDEX: &str = "items.json";

/// The JSON document: `{"items": [...]}` with every field.
pub fn render_json(items: &[DictationItem]) -> String {
    let mut text = serde_json::to_string_pretty(&json!({ "items": items })).unwrap_or_default();
    text.push('\n');
    text
}

/// Markdown: a heading per item with its facts, then the text.
pub fn render_md(items: &[DictationItem]) -> String {
    let mut out = String::from("# Dettivo dictations\n");
    for item in items {
        out.push_str(&format!("\n## {}\n\n", item.title));
        out.push_str(&format!(
            "- id: `{}`\n- created: {}\n- app: {}\n- engine: {}/{}\n- duration: {} s\n- source: {}\n",
            item.id,
            item.created_at,
            if item.app_name.is_empty() {
                &item.app_id
            } else {
                &item.app_name
            },
            item.stt_provider,
            item.stt_model,
            item.duration_seconds(),
            item.source_kind.as_str()
        ));
        if let Some(of) = &item.rerun_of_item_id {
            out.push_str(&format!("- re-run of: `{of}`\n"));
        }
        if item.status == ItemStatus::Failed {
            out.push_str(&format!(
                "- failed: {} {}\n",
                item.error_code.as_deref().unwrap_or(""),
                item.error_message.as_deref().unwrap_or("")
            ));
        }
        out.push('\n');
        out.push_str(&item.final_text);
        out.push('\n');
    }
    out
}

/// Plain text: a timestamp line then the text, blank line between items.
pub fn render_txt(items: &[DictationItem]) -> String {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}  {}\n{}\n",
            item.created_at,
            if item.app_name.is_empty() {
                &item.app_id
            } else {
                &item.app_name
            },
            item.final_text
        ));
    }
    out
}

/// The archive: `items.json` plus `dictations/<id>/microphone.wav` for
/// every item with retained audio.
pub fn render_zip(items: &[DictationItem]) -> Vec<u8> {
    let mut w = zip::Writer::new();
    w.add(ARCHIVE_INDEX, render_json(items).as_bytes());
    for item in items {
        if let Some(path) = &item.audio_path {
            if let Ok(bytes) = std::fs::read(path) {
                w.add(&format!("dictations/{}/microphone.wav", item.id), &bytes);
            }
        }
    }
    w.finish()
}

/// Renders `items` in `format`.
pub fn render(format: Format, items: &[DictationItem]) -> Vec<u8> {
    match format {
        Format::Json => render_json(items).into_bytes(),
        Format::Md => render_md(items).into_bytes(),
        Format::Txt => render_txt(items).into_bytes(),
        Format::Zip => render_zip(items),
    }
}

/// Restores an archive: every item in `items.json` that the store does
/// not have yet is inserted with its id, and its audio is adopted into the
/// item directory. Returns the restored ids, newest first.
pub fn restore_zip(
    store: &Store,
    artifacts: &Artifacts,
    retention: &Retention,
    bytes: &[u8],
) -> Result<Vec<String>, StoreError> {
    let entries = zip::read(bytes).map_err(StoreError::InvalidQuery)?;
    let index = entries
        .iter()
        .find(|(name, _)| name == ARCHIVE_INDEX)
        .ok_or_else(|| StoreError::InvalidQuery(format!("archive has no {ARCHIVE_INDEX}")))?;
    let doc: serde_json::Value = serde_json::from_slice(&index.1)
        .map_err(|e| StoreError::InvalidQuery(format!("{ARCHIVE_INDEX}: {e}")))?;
    let items: Vec<DictationItem> = serde_json::from_value(doc["items"].clone())
        .map_err(|e| StoreError::InvalidQuery(format!("{ARCHIVE_INDEX}: {e}")))?;
    let mut restored = Vec::new();
    for mut item in items {
        // The id becomes a directory name; only a canonical lowercase UUID
        // from the archive is trusted there.
        let canonical = uuid::Uuid::parse_str(&item.id)
            .ok()
            .map(|u| u.as_hyphenated().to_string());
        if canonical.as_deref() != Some(item.id.as_str()) {
            return Err(StoreError::InvalidQuery(format!(
                "{ARCHIVE_INDEX}: item id {:?} is not a canonical UUID",
                item.id
            )));
        }
        if store.get(&item.id)?.is_some() {
            continue;
        }
        item.audio_path = None;
        let audio = entries
            .iter()
            .find(|(name, _)| *name == format!("dictations/{}/microphone.wav", item.id));
        if let Some((_, wav)) = audio {
            if retention.keep_audio {
                let dir = artifacts.dir(&item.id);
                std::fs::create_dir_all(&dir)?;
                let target = dir.join(crate::retention::AUDIO_FILE);
                std::fs::write(&target, wav)?;
                item.audio_path = Some(target.to_string_lossy().into_owned());
            }
        }
        store.insert(&item)?;
        artifacts.write_metadata(&item, retention)?;
        restored.push(item.id);
    }
    Ok(restored)
}

/// Reads an audio file's bytes for an archive entry check (tests).
pub fn audio_bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_parse_and_name_their_payloads() {
        assert_eq!(Format::parse("markdown"), Some(Format::Md));
        assert_eq!(Format::parse("md"), Some(Format::Md));
        assert_eq!(Format::parse("srt"), None);
        assert_eq!(Format::Json.content_type(), "application/json");
        assert_eq!(Format::Zip.filename("dictations"), "dictations.zip");
        let empty = render(Format::Json, &[]);
        assert_eq!(String::from_utf8(empty).unwrap(), "{\n  \"items\": []\n}\n");
        assert_eq!(render_txt(&[]), "");
        assert_eq!(render_md(&[]), "# Dettivo dictations\n");
    }
}
