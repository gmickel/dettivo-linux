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

    /// The largest vocabulary the configuration accepts still fits the
    /// recognize header, so no accepted list breaks Parakeet dictation.
    #[test]
    fn every_accepted_vocabulary_fits_the_recognize_header() {
        use dettivo_core::config::vocabulary_schema::{
            MAX_ALIASES, MAX_ENTRIES, MAX_VOCABULARY_BYTES, limit_error,
        };
        // Spellings of control characters: each takes six bytes as JSON.
        let entry = |chars: usize| VocabularyEntry::Entry {
            term: "x".repeat(64),
            sounds_like: vec!["\u{1}".repeat(chars); MAX_ALIASES],
        };
        // The longest spelling accepted, searched up to a length that
        // would overrun the header many times over.
        let chars = (1..=1024)
            .take_while(|&n| limit_error(&vec![entry(n); MAX_ENTRIES]).is_none())
            .last()
            .unwrap();
        let entries = vec![entry(chars); MAX_ENTRIES];
        assert!(serde_json::to_vec(&entries).unwrap().len() <= MAX_VOCABULARY_BYTES);
        let params = dettivo_engine_proto::RecognizeParams {
            language: "en".into(),
            prompt: None,
            timestamps: true,
            vocabulary: vocabulary_bias(&entries, VocabularyBoost::Normal),
        };
        let header = serde_json::to_vec(&params).unwrap().len();
        assert!(
            header < dettivo_engine_proto::frame::MAX_HEADER_BYTES as usize,
            "{header}"
        );
    }
}
