//! `[dictation] vocabulary` entries: a term on its own (`"Dettivo"`) or a
//! table that also names how the term tends to be misheard
//! (`{ term = "Clef", sounds_like = ["clay"] }`). Both forms sit in one
//! list. The engine prompt takes the terms; the Enhanced prompt also takes
//! the aliases (ADR 0086).

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// The most entries a vocabulary holds.
pub const MAX_ENTRIES: usize = 500;
/// The longest term, in characters.
pub const MAX_TERM_CHARS: usize = 64;
/// The most `sounds_like` spellings one term carries.
pub const MAX_ALIASES: usize = 16;

/// One vocabulary entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum VocabularyEntry {
    /// A term on its own, the form every vocabulary had before aliases.
    Word(String),
    /// A term with the spellings it tends to be transcribed as.
    Entry {
        /// The spelling the user wants.
        term: String,
        /// How the term tends to be misheard.
        sounds_like: Vec<String>,
    },
}

impl VocabularyEntry {
    /// The spelling the user wants.
    pub fn term(&self) -> &str {
        match self {
            Self::Word(term) | Self::Entry { term, .. } => term,
        }
    }

    /// The spellings the term tends to be transcribed as.
    pub fn aliases(&self) -> &[String] {
        match self {
            Self::Word(_) => &[],
            Self::Entry { sounds_like, .. } => sounds_like,
        }
    }
}

impl From<&str> for VocabularyEntry {
    fn from(term: &str) -> Self {
        Self::Word(term.to_string())
    }
}

/// The table form, read strictly so a misspelt key is refused by name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    term: String,
    #[serde(default)]
    sounds_like: Vec<String>,
}

impl<'de> Deserialize<'de> for VocabularyEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = VocabularyEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a term or a { term, sounds_like } table")
            }

            fn visit_str<E: de::Error>(self, term: &str) -> Result<Self::Value, E> {
                Ok(VocabularyEntry::Word(term.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let table = Table::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(VocabularyEntry::Entry {
                    term: table.term,
                    sounds_like: table.sounds_like,
                })
            }
        }

        deserializer.deserialize_any(EntryVisitor)
    }
}

/// The terms of `entries`, in list order.
pub fn terms(entries: &[VocabularyEntry]) -> Vec<String> {
    entries.iter().map(|e| e.term().to_string()).collect()
}

/// `entries` without a later entry whose term matches an earlier one
/// regardless of case: the first spelling wins.
pub fn dedup(entries: Vec<VocabularyEntry>) -> Vec<VocabularyEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|e| seen.insert(e.term().to_lowercase()))
        .collect()
}

/// The first limit `entries` breaks, naming the entry.
pub fn limit_error(entries: &[VocabularyEntry]) -> Option<String> {
    if entries.len() > MAX_ENTRIES {
        return Some(format!(
            "dictation.vocabulary has {} entries; it holds at most {MAX_ENTRIES}",
            entries.len()
        ));
    }
    for entry in entries {
        let chars = entry.term().chars().count();
        if chars > MAX_TERM_CHARS {
            return Some(format!(
                "dictation.vocabulary: {:?} is {chars} characters; a term holds at most {MAX_TERM_CHARS}",
                entry.term()
            ));
        }
        let aliases = entry.aliases().len();
        if aliases > MAX_ALIASES {
            return Some(format!(
                "dictation.vocabulary: {:?} has {aliases} sounds_like spellings; a term takes at most {MAX_ALIASES}",
                entry.term()
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Doc {
        vocabulary: Vec<VocabularyEntry>,
    }

    fn read(text: &str) -> Result<Vec<VocabularyEntry>, String> {
        toml::from_str::<Doc>(text)
            .map(|d| d.vocabulary)
            .map_err(|e| e.message().to_string())
    }

    #[test]
    fn plain_strings_and_tables_share_one_list() {
        let entries = read(
            r#"vocabulary = ["Dettivo", { term = "Clef", sounds_like = ["clay", "cleff"] }, { term = "SapienXT" }]"#,
        )
        .unwrap();
        assert_eq!(entries[0], VocabularyEntry::Word("Dettivo".into()));
        assert_eq!(entries[1].term(), "Clef");
        assert_eq!(entries[1].aliases(), ["clay", "cleff"]);
        assert_eq!(entries[2].term(), "SapienXT");
        assert!(entries[2].aliases().is_empty());
        assert!(entries[0].aliases().is_empty());
        assert_eq!(terms(&entries), ["Dettivo", "Clef", "SapienXT"]);
    }

    #[test]
    fn a_misspelt_or_missing_key_and_a_wrong_type_are_refused() {
        let err = read(r#"vocabulary = [{ term = "Clef", sound_like = ["clay"] }]"#).unwrap_err();
        assert!(err.contains("unknown field `sound_like`"), "{err}");
        let err = read(r#"vocabulary = [{ sounds_like = ["clay"] }]"#).unwrap_err();
        assert!(err.contains("missing field `term`"), "{err}");
        let err = read("vocabulary = [5]").unwrap_err();
        assert!(
            err.contains("a term or a { term, sounds_like } table"),
            "{err}"
        );
    }

    #[test]
    fn duplicates_go_regardless_of_case_and_the_first_spelling_wins() {
        let entries = dedup(vec![
            "SapienXT".into(),
            VocabularyEntry::Entry {
                term: "sapienxt".into(),
                sounds_like: vec!["sapient".into()],
            },
            "Clef".into(),
            "CLEF".into(),
        ]);
        assert_eq!(entries, vec!["SapienXT".into(), "Clef".into()]);
    }

    #[test]
    fn each_limit_names_the_entry_that_breaks_it() {
        let ok: Vec<VocabularyEntry> = (0..MAX_ENTRIES)
            .map(|i| format!("t{i}").as_str().into())
            .collect();
        assert_eq!(limit_error(&ok), None);
        let mut over = ok.clone();
        over.push("one more".into());
        assert_eq!(
            limit_error(&over).as_deref(),
            Some("dictation.vocabulary has 501 entries; it holds at most 500")
        );
        let long = "x".repeat(MAX_TERM_CHARS + 1);
        let err = limit_error(&[long.as_str().into()]).unwrap();
        assert!(err.contains(&format!("{long:?} is 65 characters")), "{err}");
        assert_eq!(
            limit_error(&["ü".repeat(MAX_TERM_CHARS).as_str().into()]),
            None
        );
        let aliased = VocabularyEntry::Entry {
            term: "Clef".into(),
            sounds_like: vec!["clay".into(); MAX_ALIASES + 1],
        };
        assert_eq!(
            limit_error(&[aliased]).as_deref(),
            Some(
                "dictation.vocabulary: \"Clef\" has 17 sounds_like spellings; a term takes at most 16"
            )
        );
    }
}
