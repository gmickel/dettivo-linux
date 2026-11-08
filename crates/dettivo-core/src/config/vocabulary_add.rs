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
    /// The indices of the entries whose new spellings joined the entry the
    /// file already held for their term ([`add_or_extend`] only).
    pub extended: Vec<usize>,
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
    apply(text, entries, false)
}

/// As [`add`], except that an entry whose term the file already holds
/// gives that entry the spellings it lacks instead of being skipped
/// (`dettivo vocabulary suggest --apply`, ADR 0104). The file's entry keeps
/// its text, its comments and its spellings; the new ones follow them.
pub fn add_or_extend(text: &str, entries: &[VocabularyEntry]) -> Result<Added, AddError> {
    apply(text, entries, true)
}

/// What one new entry does to the list.
enum Change {
    Add,
    /// The file's entry at this index gains these spellings.
    Extend(usize, Vec<String>),
}

fn apply(text: &str, entries: &[VocabularyEntry], merge: bool) -> Result<Added, AddError> {
    validate(text).map_err(AddError::Invalid)?;
    let mut list = written(text);
    // The first entry of a term is the one the loader keeps.
    let mut held: HashMap<String, usize> = HashMap::new();
    for (i, e) in list.iter().enumerate() {
        held.entry(term_key(e.term())).or_insert(i);
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut changes = Vec::new();
    let mut skipped = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let key = term_key(entry.term());
        if let Some(&repeats) = seen.get(&key) {
            skipped.push(Skipped {
                index,
                repeats: Some(repeats),
            });
            continue;
        }
        seen.insert(key.clone(), index);
        let Some(&at) = held.get(&key) else {
            changes.push((index, Change::Add));
            continue;
        };
        let spellings = if merge {
            missing_spellings(list[at].aliases(), entry.aliases())
        } else {
            Vec::new()
        };
        if spellings.is_empty() {
            skipped.push(Skipped {
                index,
                repeats: None,
            });
        } else {
            changes.push((index, Change::Extend(at, spellings)));
        }
    }
    // Each new entry is counted in the form the file will load it.
    let tables = writes_tables(text);
    let mut refused = Vec::new();
    for (index, change) in &changes {
        let entry = &entries[*index];
        let term = entry.term();
        let (chars, aliases) = match change {
            Change::Add => {
                let loaded = if tables {
                    VocabularyEntry::Entry {
                        term: term.to_string(),
                        sounds_like: entry.aliases().to_vec(),
                    }
                } else {
                    entry.clone()
                };
                list.push(loaded);
                (term.chars().count(), entry.aliases().len())
            }
            Change::Extend(at, spellings) => {
                let mut sounds_like = list[*at].aliases().to_vec();
                sounds_like.extend(spellings.iter().cloned());
                let aliases = sounds_like.len();
                list[*at] = VocabularyEntry::Entry {
                    term: list[*at].term().to_string(),
                    sounds_like,
                };
                (0, aliases)
            }
        };
        let bytes = json_len(&list);
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
        refused.push((*index, why));
    }
    if !refused.is_empty() {
        return Err(AddError::Refused(refused));
    }
    let mut added = Vec::new();
    let mut extended = Vec::new();
    let mut extensions = Vec::new();
    for (index, change) in changes {
        match change {
            Change::Add => added.push(index),
            Change::Extend(at, spellings) => {
                extended.push(index);
                extensions.push((at, spellings));
            }
        }
    }
    let mut out = text.to_string();
    if !extensions.is_empty() {
        out = extend(&out, &extensions).map_err(AddError::Invalid)?;
    }
    if !added.is_empty() {
        let new: Vec<&VocabularyEntry> = added.iter().map(|&i| &entries[i]).collect();
        out = append(&out, &new).map_err(AddError::Invalid)?;
    }
    Ok(Added {
        text: out,
        added,
        extended,
        skipped,
        entries: list.len(),
    })
}

/// The spellings of `new` that `held` lacks, compared in lower case, each
/// once and in order.
fn missing_spellings(held: &[String], new: &[String]) -> Vec<String> {
    let mut known: Vec<String> = held.iter().map(|s| s.to_lowercase()).collect();
    let mut out = Vec::new();
    for spelling in new {
        let key = spelling.to_lowercase();
        if !known.contains(&key) {
            known.push(key);
            out.push(spelling.clone());
        }
    }
    out
}

/// `text` with each `(index, spellings)` added to the `sounds_like` of the
/// file's entry at that index: a plain term becomes `{ term, sounds_like }`
/// in its place, keeping the comment beside it.
fn extend(text: &str, extensions: &[(usize, Vec<String>)]) -> Result<String, ValidationError> {
    let invalid = |message: String| ValidationError {
        key: Some(KEY.to_string()),
        line: None,
        message,
    };
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|e| invalid(e.message().to_string()))?;
    let lost = || invalid(format!("{KEY}: the entry to extend moved"));
    let vocabulary = doc
        .get_mut("dictation")
        .and_then(|d| d.get_mut("vocabulary"))
        .ok_or_else(lost)?;
    let not_a_list = || invalid(format!("{KEY}: sounds_like is not a list"));
    for (at, spellings) in extensions {
        let held = match &mut *vocabulary {
            Item::Value(toml_edit::Value::Array(array)) => {
                let value = array.get_mut(*at).ok_or_else(lost)?;
                if let Some(term) = value.as_str().map(str::to_string) {
                    // A plain term becomes a table in its place.
                    let mut table = toml_edit::InlineTable::new();
                    table.insert("term", term.into());
                    table.insert("sounds_like", Array::new().into());
                    let mut replaced = toml_edit::Value::InlineTable(table);
                    *replaced.decor_mut() = value.decor().clone();
                    *value = replaced;
                }
                let table = value.as_inline_table_mut().ok_or_else(lost)?;
                if !table.contains_key("sounds_like") {
                    table.insert("sounds_like", Array::new().into());
                }
                table.get_mut("sounds_like")
            }
            Item::ArrayOfTables(tables) => {
                let table = tables.get_mut(*at).ok_or_else(lost)?;
                if !table.contains_key("sounds_like") {
                    table.insert("sounds_like", toml_edit::value(Array::new()));
                }
                table.get_mut("sounds_like").and_then(Item::as_value_mut)
            }
            _ => return Err(lost()),
        };
        let array = held
            .and_then(toml_edit::Value::as_array_mut)
            .ok_or_else(not_a_list)?;
        let before = array.clone();
        for spelling in spellings {
            array.push(spelling.as_str());
        }
        file_syntax::keep_layout(&before, array);
    }
    let out = doc.to_string();
    validate(&out)?;
    Ok(out)
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
