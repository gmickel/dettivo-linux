//! `dettivo-qa bench vocabulary` (ADR 0087): whether a vocabulary helps.
//! The fixture set under `fixtures/vocabulary/` holds target clips (a
//! term in a natural sentence), distractor clips (a sound-alike word that
//! must not become a term) and guard clips (general speech). Each speech
//! engine transcribes every clip through its CLI mode with the vocabulary
//! off and on, and each pass is scored: term recall over the target
//! clips, terms inserted into distractor and guard clips, and the word
//! error rate of the guard clips. A term counts only as a whole-word,
//! case-insensitive match of its own spelling; an alias is never a hit.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::pack::wer::{wer, words};

/// The fixture set, relative to the repository root.
pub const FIXTURE_DIR: &str = "crates/dettivo-qa/fixtures/vocabulary";

/// One vocabulary entry of the fixture set, the `config.toml` shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The spelling wanted.
    pub term: String,
    /// How the term tends to be misheard.
    #[serde(default)]
    pub sounds_like: Vec<String>,
}

/// What a clip is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Carries a term.
    Target,
    /// Carries a sound-alike word that must not become a term.
    Distractor,
    /// General speech with no term.
    Guard,
}

/// One clip: `<id>.wav` beside the definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// The file stem.
    pub id: String,
    /// What the clip is for.
    pub kind: Kind,
    /// The term a target clip carries.
    #[serde(default)]
    pub term: Option<String>,
    /// What was meant.
    pub text: String,
}

/// `set.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Set {
    /// The vocabulary a pass with the vocabulary on primes the engine with.
    pub vocabulary: Vec<Entry>,
    /// Every clip.
    pub clips: Vec<Clip>,
}

impl Set {
    /// Reads `set.json` from `dir`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("set.json");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The vocabulary's terms, in list order.
    pub fn terms(&self) -> Vec<String> {
        self.vocabulary.iter().map(|e| e.term.clone()).collect()
    }
}

/// One pass's score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VocabularyScore {
    /// Target clips whose term came back, over the target clips.
    pub term_recall: (u32, u32),
    /// Terms in distractor and guard transcripts that the speaker never said.
    pub false_insertions: u32,
    /// Word error rate over the guard clips together.
    pub guard_wer: f32,
}

/// How often the words of `phrase` occur in a row in `text`.
fn occurrences(text: &[String], phrase: &[String]) -> u32 {
    if phrase.is_empty() || phrase.len() > text.len() {
        return 0;
    }
    let found = text.windows(phrase.len()).filter(|w| *w == phrase).count();
    u32::try_from(found).unwrap_or(u32::MAX)
}

/// Scores `transcripts` (clip id and text) against `set`; a clip with no
/// transcript counts as an empty one.
pub fn score(set: &Set, transcripts: &[(String, String)]) -> VocabularyScore {
    let heard = |id: &str| {
        let text = transcripts
            .iter()
            .find(|(c, _)| c == id)
            .map(|(_, t)| t.as_str());
        words(text.unwrap_or(""))
    };
    let terms: Vec<Vec<String>> = set.vocabulary.iter().map(|e| words(&e.term)).collect();
    let (mut hits, mut targets, mut inserted) = (0, 0, 0);
    let (mut guard_ref, mut guard_hyp) = (Vec::new(), Vec::new());
    for clip in &set.clips {
        let hyp = heard(&clip.id);
        match clip.kind {
            Kind::Target => {
                targets += 1;
                let term = words(clip.term.as_deref().unwrap_or(""));
                hits += u32::from(occurrences(&hyp, &term) > 0);
            }
            Kind::Distractor | Kind::Guard => {
                let said = words(&clip.text);
                for term in &terms {
                    inserted += occurrences(&hyp, term).saturating_sub(occurrences(&said, term));
                }
                if clip.kind == Kind::Guard {
                    guard_ref.extend(said);
                    guard_hyp.extend(hyp);
                }
            }
        }
    }
    VocabularyScore {
        term_recall: (hits, targets),
        false_insertions: inserted,
        guard_wer: (wer(&guard_ref, &guard_hyp) * 10_000.0).round() as f32 / 10_000.0,
    }
}

/// The prompt the vocabulary primes the engine with when the engine takes
/// one: the terms joined as `dettivo_speech::vocabulary_prompt` joins
/// them, never the aliases. `None` for an engine that takes no vocabulary.
pub fn prompt_args(provider: &str, terms: &[String]) -> Option<Vec<String>> {
    match provider {
        "whisper" => Some(vec!["--prompt".into(), format!("{}.", terms.join(", "))]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> Set {
        let clip = |id: &str, kind, term: Option<&str>, text: &str| Clip {
            id: id.into(),
            kind,
            term: term.map(str::to_string),
            text: text.into(),
        };
        Set {
            vocabulary: vec![
                Entry {
                    term: "Clef".into(),
                    sounds_like: vec!["clay".into()],
                },
                Entry {
                    term: "Sapien XT".into(),
                    sounds_like: vec![],
                },
            ],
            clips: vec![
                clip("t1", Kind::Target, Some("Clef"), "The Clef release ships."),
                clip("t2", Kind::Target, Some("Clef"), "Ask the Clef team."),
                clip("t3", Kind::Target, Some("Sapien XT"), "Sapien XT signed."),
                clip("d1", Kind::Distractor, None, "She shaped the clay."),
                clip("g1", Kind::Guard, None, "Rain is expected."),
            ],
        }
    }

    fn heard(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn recall_counts_whole_word_case_insensitive_term_matches_and_never_an_alias() {
        let score = score(
            &set(),
            &heard(&[
                ("t1", "the CLEF release ships."),
                ("t2", "Ask the clay team."),
                ("t3", "sapien xt signed"),
                ("d1", "She shaped the clay."),
                ("g1", "Rain is expected."),
            ]),
        );
        assert_eq!(score.term_recall, (2, 3));
        assert_eq!(score.false_insertions, 0);
        assert_eq!(score.guard_wer, 0.0);
        let partial = super::score(
            &set(),
            &heard(&[("t1", "The Clefs release."), ("t3", "Sapien signed")]),
        );
        assert_eq!(
            partial.term_recall,
            (0, 3),
            "Clefs and a half term are misses"
        );
    }

    #[test]
    fn a_term_in_a_distractor_or_guard_transcript_is_a_false_insertion() {
        let score = score(
            &set(),
            &heard(&[
                ("d1", "She shaped the Clef."),
                ("g1", "Rain is Clef expected, Sapien XT."),
            ]),
        );
        assert_eq!(score.false_insertions, 3);
        assert_eq!(score.term_recall, (0, 3), "a missing transcript is empty");
        assert_eq!(score.guard_wer, 1.0, "three words inserted over three said");
    }

    #[test]
    fn only_an_engine_that_takes_a_prompt_gets_the_terms_and_never_the_aliases() {
        let terms = set().terms();
        assert_eq!(
            prompt_args("whisper", &terms),
            Some(vec!["--prompt".into(), "Clef, Sapien XT.".into()])
        );
        assert_eq!(prompt_args("parakeet", &terms), None);
    }

    #[test]
    fn the_checked_in_set_has_a_clip_for_every_entry_and_three_targets_a_term() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/vocabulary");
        let set = Set::load(&dir).unwrap();
        for entry in &set.vocabulary {
            let targets = set
                .clips
                .iter()
                .filter(|c| c.term.as_deref() == Some(&entry.term));
            assert_eq!(targets.count(), 3, "{}", entry.term);
        }
        for kind in [Kind::Distractor, Kind::Guard] {
            assert!(set.clips.iter().any(|c| c.kind == kind));
        }
        for clip in &set.clips {
            assert!(
                dir.join(format!("{}.wav", clip.id)).is_file(),
                "{}",
                clip.id
            );
        }
        let perfect: Vec<(String, String)> = set
            .clips
            .iter()
            .map(|c| (c.id.clone(), c.text.clone()))
            .collect();
        let score = score(&set, &perfect);
        assert_eq!(score.term_recall, (15, 15));
        assert_eq!((score.false_insertions, score.guard_wer), (0, 0.0));
    }
}
