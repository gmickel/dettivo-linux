//! The catalogue's per-language recommendations (ADR 0085): each speech
//! model carries `[[models.language_wer]]` rows from the recorded FLEURS
//! run (`dettivo-qa bench --step language_wer`), with the word error rate
//! and the rank the run gave it. First run and `dettivo speech providers
//! --language` read them; nothing else does, so an existing selection
//! never changes.

use serde::{Deserialize, Serialize};

use crate::catalogue::{Catalogue, ModelEntry, ModelKind};

/// One language a speech model was scored in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageWer {
    /// The language code (`de`).
    pub language: String,
    /// 1 for the model this language recommends; rates within 1.5 points
    /// tie, and the faster model takes the better rank.
    pub rank: u32,
    /// Word error rate in percent on the recorded run.
    pub wer: f64,
}

/// A model's place in one language's ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked<'a> {
    /// The model.
    pub entry: &'a ModelEntry,
    /// Its rank.
    pub rank: u32,
    /// Its word error rate.
    pub wer: f64,
}

/// The language part of a code or locale: `de_CH.UTF-8`, `de-CH` and
/// `DE` all read `de`.
pub fn language_of(code: &str) -> String {
    code.split(['_', '-', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

impl Catalogue {
    /// The speech models scored in `language`, best rank first; empty for
    /// a language the recorded run did not cover.
    pub fn ranking(&self, language: &str) -> Vec<Ranked<'_>> {
        let language = language_of(language);
        let mut out: Vec<Ranked<'_>> = self
            .models
            .iter()
            .filter(|m| m.kind == ModelKind::Stt)
            .filter_map(|m| {
                let row = m.language_wer.iter().find(|r| r.language == language)?;
                Some(Ranked {
                    entry: m,
                    rank: row.rank,
                    wer: row.wer,
                })
            })
            .collect();
        out.sort_by_key(|r| r.rank);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_reads_as_its_language() {
        for code in ["de", "de_CH.UTF-8", "de-CH", "DE", "de@euro"] {
            assert_eq!(language_of(code), "de", "{code}");
        }
    }

    #[test]
    fn the_builtin_catalogue_ranks_each_scored_language_from_one() {
        let catalogue = Catalogue::builtin();
        let ranking = catalogue.ranking("de_DE.UTF-8");
        assert!(!ranking.is_empty(), "German is scored");
        let ranks: Vec<u32> = ranking.iter().map(|r| r.rank).collect();
        assert_eq!(ranks, (1..=ranks.len() as u32).collect::<Vec<_>>());
        assert!(
            ranking.iter().all(|r| !r.entry.english_only),
            "an English-only model is never ranked for German"
        );
        assert!(catalogue.ranking("xx").is_empty());
    }
}
