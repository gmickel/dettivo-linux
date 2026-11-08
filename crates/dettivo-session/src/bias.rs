//! The vocabulary as the engines take it: for Parakeet every term with its
//! `sounds_like` spellings and the boost of the configured level (ADR
//! 0088), for Whisper the prompt's terms its output is checked against,
//! with their spellings (ADR 0095).

use dettivo_core::config::vocabulary_schema::{VocabularyBoost, VocabularyEntry};
use dettivo_engine_proto::{BiasTerm, VocabularyBias};

/// The bias a Parakeet request carries; `None` with no entries or the
/// boost off, so the engine decodes exactly as it does without one.
pub fn vocabulary_bias(
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

/// The terms a Whisper request checks its prompted output against: the
/// prompt's own, since only a term in the prompt can be read back from it,
/// each with its `sounds_like` spellings from `entries`, which a lone name
/// may sound like. Whisper ignores the boost; `None` without terms.
pub fn checked_terms(
    prompt_terms: &[String],
    entries: &[VocabularyEntry],
) -> Option<VocabularyBias> {
    (!prompt_terms.is_empty()).then(|| VocabularyBias {
        terms: prompt_terms
            .iter()
            .map(|t| BiasTerm {
                term: t.clone(),
                aliases: entries
                    .iter()
                    .find(|e| e.term() == t)
                    .map(|e| e.aliases().to_vec())
                    .unwrap_or_default(),
            })
            .collect(),
        boost: 0.0,
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
        let checked = checked_terms(&["Clef".into()], &entries).unwrap();
        assert_eq!(checked.terms.len(), 1);
        assert_eq!(checked.terms[0].aliases, vec!["clay".to_string()]);
        assert_eq!(checked_terms(&[], &entries), None);
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
