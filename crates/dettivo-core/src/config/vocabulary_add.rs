//! Adding entries to `[dictation] vocabulary` in the file text (fn-95,
//! ADR 0101): each new entry goes after the last one in the file's own
//! array, so every entry already written, a repeat the loader drops
//! included, keeps its text and its comments. A new entry whose term the
//! file or an earlier new entry already holds, in any case, is skipped;
//! the limits apply to the list as it would be written, and an entry that
//! breaks one refuses the whole addition by its index.

use std::collections::HashMap;

use dettivo_proto::methods::config::ValidationError;
use serde_json::Value;
use toml_edit::{Array, DocumentMut, Item, Table};

use super::file_syntax;
use super::validate::validate;
use super::vocabulary_schema::{
    MAX_ALIASES, MAX_ENTRIES, MAX_TERM_CHARS, MAX_VOCABULARY_BYTES, VocabularyEntry, term_key,
};

const KEY: &str = "dictation.vocabulary";

/// A new entry skipped as a duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Its index among the new entries.
    pub index: usize,
    /// The earlier new entry it repeats; `None` when the file held the term.
    pub repeats: Option<usize>,
}

/// What an addition changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The file text with the new entries; the input when none was added.
    pub text: String,
    /// The indices of the entries added.
    pub added: Vec<usize>,
    /// The entries skipped as duplicates.
    pub skipped: Vec<Skipped>,
    /// How many entries the file's list holds after the addition.
    pub entries: usize,
}

/// Why nothing was added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// The file, or the file the addition would write, does not validate.
    Invalid(ValidationError),
    /// Each new entry that breaks a limit, by index, with the limit.
    Refused(Vec<(usize, String)>),
}

/// The entries the file writes, before the loader drops repeats.
fn written(text: &str) -> Vec<VocabularyEntry> {
    let table: toml::Table = toml::from_str(text).unwrap_or_default();
    table
        .get("dictation")
        .and_then(|d| d.get("vocabulary"))
        .cloned()
        .and_then(|v| v.try_into().ok())
        .unwrap_or_default()
}

/// Whether the file writes its vocabulary as `[[dictation.vocabulary]]`
/// tables, where a new term on its own loads as a table too.
fn writes_tables(text: &str) -> bool {
    text.parse::<DocumentMut>().is_ok_and(|doc| {
        doc.get("dictation")
            .and_then(|d| d.get("vocabulary"))
            .is_some_and(Item::is_array_of_tables)
    })
}

fn json_len(value: &(impl serde::Serialize + ?Sized)) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |json| json.len())
}

/// The file text with `entries` added after the vocabulary it holds.
pub fn add(text: &str, entries: &[VocabularyEntry]) -> Result<Added, AddError> {
    validate(text).map_err(AddError::Invalid)?;
    let mut list = written(text);
    let mut seen: HashMap<String, Option<usize>> =
        list.iter().map(|e| (term_key(e.term()), None)).collect();
    let mut added = Vec::new();
    let mut skipped = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        match seen.get(&term_key(entry.term())) {
            Some(&repeats) => skipped.push(Skipped { index, repeats }),
            None => {
                seen.insert(term_key(entry.term()), Some(index));
                added.push(index);
            }
        }
    }
    // Each new entry is counted in the form the file will load it.
    let tables = writes_tables(text);
    let mut bytes = json_len(&list);
    let mut refused = Vec::new();
    for &index in &added {
        let entry = &entries[index];
        let term = entry.term();
        let loaded = if tables {
            VocabularyEntry::Entry {
                term: term.to_string(),
                sounds_like: entry.aliases().to_vec(),
            }
        } else {
            entry.clone()
        };
        bytes = bytes.saturating_add(json_len(&loaded) + usize::from(!list.is_empty()));
        list.push(loaded);
        let chars = term.chars().count();
        let aliases = entry.aliases().len();
        let why = if chars > MAX_TERM_CHARS {
            format!("{term:?} is {chars} characters; a term holds at most {MAX_TERM_CHARS}")
        } else if aliases > MAX_ALIASES {
            format!(
                "{term:?} has {aliases} sounds_like spellings; a term takes at most {MAX_ALIASES}"
            )
        } else if list.len() > MAX_ENTRIES {
            format!(
                "{term:?} would be entry {}; a vocabulary holds at most {MAX_ENTRIES}",
                list.len()
            )
        } else if bytes > MAX_VOCABULARY_BYTES {
            format!(
                "{term:?} would take the vocabulary to {bytes} bytes; it holds at most {MAX_VOCABULARY_BYTES}"
            )
        } else {
            continue;
        };
        refused.push((index, why));
    }
    if !refused.is_empty() {
        return Err(AddError::Refused(refused));
    }
    let text = if added.is_empty() {
        text.to_string()
    } else {
        let new: Vec<&VocabularyEntry> = added.iter().map(|&i| &entries[i]).collect();
        append(text, &new).map_err(AddError::Invalid)?
    };
    Ok(Added {
        text,
        added,
        skipped,
        entries: list.len(),
    })
}

/// `text` with `new` after the last entry of its vocabulary, in the form
/// the file writes it: an inline array or `[[dictation.vocabulary]]`
/// tables. Every entry it has keeps its text; a file without the key gets
/// it, and any other value is refused without a write.
fn append(text: &str, new: &[&VocabularyEntry]) -> Result<String, ValidationError> {
    let invalid = |message: String| ValidationError {
        key: Some(KEY.to_string()),
        line: None,
        message,
    };
    let not_a_list = || invalid(format!("{KEY}: not a list of entries"));
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|e| invalid(e.message().to_string()))?;
    let values = || {
        new.iter()
            .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
            .collect::<Vec<_>>()
    };
    match doc
        .get_mut("dictation")
        .and_then(|d| d.get_mut("vocabulary"))
    {
        // `Item::get_mut` inserts `Item::None` for a missing key in a table
        // that exists, so both read as no vocabulary written yet.
        None | Some(Item::None) => return super::edit::set(text, KEY, &Value::Array(values())),
        Some(Item::Value(toml_edit::Value::Array(array))) => {
            let items = file_syntax::list(&values()).ok_or_else(not_a_list)?;
            let old = array.clone();
            for item in items.iter() {
                array.push(item.clone());
            }
            file_syntax::keep_layout(&old, array);
        }
        Some(Item::ArrayOfTables(tables)) => {
            for entry in new {
                let mut table = Table::new();
                table.insert("term", toml_edit::value(entry.term()));
                if !entry.aliases().is_empty() {
                    let spellings: Array = entry.aliases().iter().map(String::as_str).collect();
                    table.insert("sounds_like", toml_edit::value(spellings));
                }
                tables.push(table);
            }
        }
        Some(_) => return Err(not_a_list()),
    }
    let out = doc.to_string();
    validate(&out)?;
    Ok(out)
}

#[cfg(test)]
#[path = "vocabulary_add_tests.rs"]
mod tests;
