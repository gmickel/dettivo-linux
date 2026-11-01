//! The vocabulary as Parakeet takes it (ADR 0088): every term with its
//! `sounds_like` spellings and the boost of the configured level.

use dettivo_core::config::vocabulary_schema::{VocabularyBoost, VocabularyEntry};
use dettivo_engine_proto::{BiasTerm, VocabularyBias};

/// The bias a Parakeet request carries; `None` with no entries or the
/// boost off, so the engine decodes exactly as it does without one.
pub(crate) fn vocabulary_bias(
    entries: &[VocabularyEntry],
    boost: VocabularyBoost,
) -> Option<VocabularyBias> {
    let nats = VocabularyBias::boost_of(boost.as_str()).unwrap_or(0.0);
    if entries.is_empty() || nats <= 0.0 {
        return None;
    }
    Some(VocabularyBias {
        terms: entries
            .iter()
            .map(|e| BiasTerm {
                term: e.term().to_string(),
                aliases: e.aliases().to_vec(),
            })
            .collect(),
        boost: nats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_and_aliases_go_with_the_level_and_off_or_empty_sends_none() {
        let entries = vec![
            VocabularyEntry::from("Dettivo"),
            VocabularyEntry::Entry {
                term: "Clef".into(),
                sounds_like: vec!["clay".into()],
            },
        ];
        let bias = vocabulary_bias(&entries, VocabularyBoost::Normal).unwrap();
        assert_eq!(bias.boost, 5.0);
        assert_eq!(bias.terms[0].term, "Dettivo");
        assert!(bias.terms[0].aliases.is_empty());
        assert_eq!(bias.terms[1].aliases, vec!["clay".to_string()]);
        assert_eq!(
            vocabulary_bias(&entries, VocabularyBoost::Strong)
                .unwrap()
                .boost,
            6.0
        );
        assert_eq!(vocabulary_bias(&entries, VocabularyBoost::Off), None);
        assert_eq!(vocabulary_bias(&[], VocabularyBoost::Normal), None);
    }
}
