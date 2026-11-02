//! `dettivo-qa bench vocabulary` (ADR 0087): whether a vocabulary helps.
//! The fixture set under `fixtures/vocabulary/` holds target clips (a
//! term in a natural sentence), distractor clips (a sound-alike word that
//! must not become a term) and guard clips (general speech). Each speech
//! engine transcribes every clip through its CLI mode with the vocabulary
//! off and on, and each pass is scored: term recall over the target
//! clips, terms inserted into distractor and guard clips, and the word
//! error rate of the guard clips. A term counts only as a whole-word,
//! case-insensitive match of its own spelling; an alias is never a hit.
//! The single-name set under `single/` holds one word said on its own: a
//! listed name, or a sound-alike that must not become one (ADR 0095).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pack::wer::{wer, words};

/// The fixture set, relative to the repository root.
pub const FIXTURE_DIR: &str = "crates/dettivo-qa/fixtures/vocabulary";
/// The single-name set, relative to [`FIXTURE_DIR`]; it is primed with
/// the vocabulary of the main set.
pub const SINGLE_DIR: &str = "single";

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
    /// The vocabulary a pass with the vocabulary on primes the engine with;
    /// the single-name set carries none of its own.
    #[serde(default)]
    pub vocabulary: Vec<Entry>,
    /// Every clip.
    pub clips: Vec<Clip>,
    /// The directory holding `set.json` and the clips.
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Set {
    /// Reads `set.json` from `dir`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("set.json");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let set: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            ..set
        })
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
    // Each guard clip is aligned on its own, so an error at the end of one
    // clip cannot cancel an error at the start of the next.
    let (mut guard_edits, mut guard_words) = (0.0, 0usize);
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
                    guard_edits += wer(&said, &hyp) * said.len().max(1) as f64;
                    guard_words += said.len();
                }
            }
        }
    }
    VocabularyScore {
        term_recall: (hits, targets),
        false_insertions: inserted,
        guard_wer: (guard_edits / guard_words.max(1) as f64 * 10_000.0).round() as f32 / 10_000.0,
    }
}

/// How the vocabulary reaches the engine: Whisper's prompt, the terms
/// joined as `dettivo_speech::vocabulary_prompt` joins them and never the
/// aliases, with the same terms and their aliases to check its output
/// against (ADR 0095);
/// Parakeet's terms with their aliases at the default boost (ADR 0088).
/// `None` for an engine that takes no vocabulary.
pub fn prompt_args(provider: &str, vocabulary: &[Entry]) -> Option<Vec<String>> {
    match provider {
        "whisper" => {
            let terms: Vec<&str> = vocabulary.iter().map(|e| e.term.as_str()).collect();
            let checked: Vec<serde_json::Value> = vocabulary
                .iter()
                .map(|e| serde_json::json!({ "term": e.term, "aliases": e.sounds_like }))
                .collect();
            Some(vec![
                "--prompt".into(),
                format!("{}.", terms.join(", ")),
                "--vocabulary".into(),
                serde_json::Value::from(checked).to_string(),
            ])
        }
        "parakeet" => {
            let terms: Vec<serde_json::Value> = vocabulary
                .iter()
                .map(|e| serde_json::json!({ "term": e.term, "aliases": e.sounds_like }))
                .collect();
            Some(vec![
                "--vocabulary".into(),
                serde_json::Value::from(terms).to_string(),
                "--boost".into(),
                "normal".into(),
            ])
        }
        _ => None,
    }
}

/// What the vocabulary cost one clip, from Parakeet's `vocabulary
/// rescoring` or Whisper's `vocabulary check` log line: candidate spans
/// and milliseconds.
pub fn rescoring(stderr: &str) -> Option<(u32, f64)> {
    let line = stderr
        .lines()
        .find(|l| l.contains("vocabulary rescoring") || l.contains("vocabulary check"))?;
    let field = |name: &str| {
        line.split_whitespace()
            .find_map(|f| f.strip_prefix(name))
            .map(|v| v.trim_matches('"').to_string())
    };
    Some((
        field("candidates=")?.parse().ok()?,
        field("rescore_ms=")
            .or_else(|| field("check_ms="))?
            .parse()
            .ok()?,
    ))
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
                clip("g2", Kind::Guard, None, "Bring an umbrella."),
            ],
            dir: PathBuf::new(),
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
                ("g2", "Bring an umbrella."),
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
        assert_eq!(
            score.guard_wer, 1.0,
            "three inserted and three missing over six said"
        );
    }

    #[test]
    fn guard_errors_in_separate_clips_never_cancel() {
        let score = score(
            &set(),
            &heard(&[("g1", "Rain is"), ("g2", "expected. Bring an umbrella.")]),
        );
        assert_eq!(
            score.guard_wer, 0.3333,
            "one deletion and one insertion over six words"
        );
    }

    #[test]
    fn whisper_gets_a_prompt_and_its_terms_and_parakeet_the_terms_with_aliases() {
        let vocabulary = set().vocabulary;
        assert_eq!(
            prompt_args("whisper", &vocabulary),
            Some(vec![
                "--prompt".into(),
                "Clef, Sapien XT.".into(),
                "--vocabulary".into(),
                r#"[{"aliases":["clay"],"term":"Clef"},{"aliases":[],"term":"Sapien XT"}]"#.into(),
            ])
        );
        assert_eq!(
            prompt_args("parakeet", &vocabulary),
            Some(vec![
                "--vocabulary".into(),
                r#"[{"aliases":["clay"],"term":"Clef"},{"aliases":[],"term":"Sapien XT"}]"#.into(),
                "--boost".into(),
                "normal".into(),
            ])
        );
        assert_eq!(prompt_args("llm", &vocabulary), None);
    }

    #[test]
    fn the_rescoring_and_check_lines_give_candidates_and_milliseconds() {
        let log = "INFO model loaded\nINFO vocabulary rescoring candidates=2 replaced=1 rescore_ms=31.4\n";
        assert_eq!(rescoring(log), Some((2, 31.4)));
        let check = "INFO vocabulary check candidates=2 suspects=1 replaced=1 check_ms=80.6";
        assert_eq!(rescoring(check), Some((2, 80.6)));
        assert_eq!(rescoring("INFO model loaded"), None);
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

    /// Two voices say each listed name alone and each lone sound-alike.
    #[test]
    fn the_single_name_set_has_two_lone_takes_a_name_and_ten_sound_alikes() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/vocabulary");
        let vocabulary = Set::load(&dir).unwrap().vocabulary;
        let set = Set {
            vocabulary,
            ..Set::load(&dir.join(SINGLE_DIR)).unwrap()
        };
        for entry in &set.vocabulary {
            let names = set
                .clips
                .iter()
                .filter(|c| c.term == Some(entry.term.clone()));
            assert_eq!(names.count(), 2, "{}", entry.term);
        }
        let alikes = set.clips.iter().filter(|c| c.kind == Kind::Distractor);
        assert_eq!(alikes.count(), 10);
        for clip in &set.clips {
            assert!(
                set.dir.join(format!("{}.wav", clip.id)).is_file(),
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
        assert_eq!((score.term_recall, score.false_insertions), ((10, 10), 0));
    }
}
