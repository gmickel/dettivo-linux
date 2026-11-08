//! `speech.vocabulary.suggest` (Linux addition, ADR 0104): the daemon
//! speaks one term with two Piper voices, alone and in a short sentence,
//! has the selected speech engine transcribe each clip with the vocabulary
//! in force and with none, and proposes the spellings it heard without one,
//! in the clips the vocabulary did not fix, as the term's `sounds_like`.
//! Nothing is written; `config.vocabulary.add` with `extend` writes them.

use serde::{Deserialize, Serialize};

/// `speech.vocabulary.suggest` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestParams {
    /// The term, spelt as it should be written.
    pub term: String,
}

/// One clip of the term and what the engine heard in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hearing {
    /// The Piper voice that spoke it.
    pub voice: String,
    /// What the voice said: the term alone or in its sentence.
    pub said: String,
    /// The engine's text with the vocabulary in force, as a dictation take
    /// hears it, after dictation's output guards.
    pub heard: String,
    /// The engine's text with no vocabulary, which the spelling is read
    /// from: the words a term's `sounds_like` is matched against.
    pub unprimed: String,
    /// Whether `heard` holds the term spelt right, as a whole word in any case.
    pub right: bool,
    /// The words `unprimed` holds for the term when `heard` missed it;
    /// `None` when it was heard right or the sentence around it did not
    /// come back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spelling: Option<String>,
}

/// `speech.vocabulary.suggest` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestResult {
    /// The term.
    pub term: String,
    /// The speech provider that transcribed the clips.
    pub provider: String,
    /// Its model.
    pub model: String,
    /// Every clip came back with the term spelt right with the vocabulary
    /// in force: it needs no `sounds_like`.
    pub recognised: bool,
    /// The misheard spellings the vocabulary does not list for the term
    /// yet, in clip order, each once.
    pub sounds_like: Vec<String>,
    /// Every clip, in a fixed order.
    pub hearings: Vec<Hearing>,
}
