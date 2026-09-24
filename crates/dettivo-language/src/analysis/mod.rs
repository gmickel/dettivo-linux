//! Meeting analysis (FR-G6, ADR 0036, ADR 0062): the finalised
//! transcript, one line per segment with its speaker, goes through the
//! same `LlmProvider` interface the Enhanced pass uses and comes back as
//! a summary, the decisions and the action items. A transcript longer
//! than `chunk_chars` is cut into parts on line boundaries and each part
//! is analysed on its own (map); a part whose answer the output budget
//! cut off, or whose prompt did not fit the context, is halved on its
//! lines and each half analysed, at most `MAX_SPLITS` deep. The parts'
//! summaries are then combined by the model in calls bounded by
//! `chunk_chars`; a merge call the budget cuts off (or whose prompt does
//! not fit) is retried on smaller groups of its summaries, down to a
//! pair, which fails. Their decisions and action items are joined in
//! code without duplicates (reduce). Every answer has its thinking block
//! stripped and is parsed strictly; a degenerate answer is asked for
//! once more with the reason, and a second one fails the run naming
//! why. The whole run shares one time budget.

pub mod parse;
pub mod prompt;
pub mod reduce;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dettivo_proto::methods::meetings::Segment;
use dettivo_proto::methods::meetings_notes::Analysis;

use crate::provider::{Answer, Finish, LlmProvider, ProviderError, RewriteRequest};

/// How many times a part is halved before a cut-off answer fails the run
/// (a part of `chunk_chars` ends up at most sixteen pieces).
pub const MAX_SPLITS: u32 = 4;

/// What a run reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Characters of transcript per call; a longer transcript runs map
    /// then reduce.
    pub chunk_chars: usize,
    /// The budget of the whole run.
    pub timeout: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            chunk_chars: 12_000,
            timeout: Duration::from_secs(60),
        }
    }
}

/// Why a run produced no analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The transcript has no words; no model was asked.
    EmptyTranscript,
    /// The provider could not be reached, was not trusted, or the run
    /// went past its budget.
    ProviderUnavailable(String),
    /// The model answered twice without a usable analysis, its answer
    /// stayed cut off after every split, or a pair of summaries could
    /// not be combined.
    InvalidOutput(String),
    /// The provider answered with an error.
    Failed(String),
    /// The run was cancelled.
    Cancelled,
}

impl Failure {
    /// The stable code the row and the events carry.
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyTranscript => "emptyTranscript",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::InvalidOutput(_) => "invalid_output",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTranscript => f.write_str("the transcript is empty"),
            Self::ProviderUnavailable(d) => write!(f, "provider unavailable: {d}"),
            Self::InvalidOutput(d) => write!(f, "the model returned no usable analysis: {d}"),
            Self::Failed(d) => write!(f, "provider failed: {d}"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for Failure {}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The analysis.
    pub analysis: Analysis,
    /// The model that answered.
    pub model: String,
    /// Calls made to the provider, repairs included.
    pub calls: u32,
    /// Pieces of transcript analysed on their own, the splits after a
    /// cut-off answer included (1 when it fit one call).
    pub parts: u32,
}

/// The transcript as the model reads it: one line per segment, the
/// speaker (or the side) in brackets, the polished text.
pub fn transcript_lines(segments: &[Segment]) -> String {
    segments
        .iter()
        .filter_map(|s| {
            let text = s.polished_text.as_deref().unwrap_or(&s.text).trim();
            if text.is_empty() {
                return None;
            }
            let who = s.speaker.as_deref().unwrap_or(match s.source_type {
                dettivo_proto::methods::meetings::SegmentSource::System => "remote",
                _ => "you",
            });
            Some(format!("[{who}] {text}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cuts `transcript` into parts of at most `chunk_chars` characters on
/// line boundaries; a line longer than a part is cut on its own.
pub fn chunk(transcript: &str, chunk_chars: usize) -> Vec<String> {
    let limit = chunk_chars.max(200);
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let push_line = |line: &str, current: &mut String, parts: &mut Vec<String>| {
        if !current.is_empty() && current.chars().count() + line.chars().count() + 1 > limit {
            parts.push(std::mem::take(current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    };
    for line in transcript.lines() {
        if line.chars().count() > limit {
            let chars: Vec<char> = line.chars().collect();
            for piece in chars.chunks(limit) {
                push_line(&piece.iter().collect::<String>(), &mut current, &mut parts);
            }
        } else {
            push_line(line, &mut current, &mut parts);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Whether `answer` was cut short: the provider says so, or the JSON
/// object never closes.
fn cut_off(answer: &Answer) -> bool {
    answer.finish == Finish::Length || parse::cut_off(&answer.text)
}

struct Run<'a> {
    provider: &'a dyn LlmProvider,
    deadline: Instant,
    cancel: Option<&'a AtomicBool>,
    calls: u32,
    chunk_chars: usize,
}

impl Run<'_> {
    fn remaining(&self) -> Result<Duration, Failure> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(Failure::Cancelled);
        }
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Failure::ProviderUnavailable(
                "the analysis did not finish inside [meetings.analysis] timeout_ms".into(),
            ));
        }
        Ok(left)
    }

    /// One call; `None` when the prompt did not fit the model's context.
    fn ask(&mut self, system: String, user: &str) -> Result<Option<Answer>, Failure> {
        let remaining = self.remaining()?;
        self.calls += 1;
        let request = RewriteRequest {
            system,
            user: user.to_string(),
        };
        match self.provider.answer(&request, remaining) {
            Ok(answer) => Ok(Some(answer)),
            Err(ProviderError::PromptTooLong(_)) => Ok(None),
            Err(ProviderError::Failed(d)) => Err(Failure::Failed(d)),
            Err(ProviderError::Timeout) => {
                Err(Failure::ProviderUnavailable("provider timed out".into()))
            }
            Err(other) => Err(Failure::ProviderUnavailable(other.to_string())),
        }
    }

    /// One analysis over `user` under `system`: the answer parsed, and
    /// one repair pass when it did not parse. `None` when the prompt did
    /// not fit or the answer was cut off: a repair cannot help there (its
    /// prompt is longer still), the caller shortens the input instead.
    fn attempt(&mut self, system: &str, user: &str) -> Result<Option<Analysis>, Failure> {
        let Some(first) = self.ask(system.to_string(), user)? else {
            return Ok(None);
        };
        if cut_off(&first) {
            return Ok(None);
        }
        let reason = match parse::parse(&first.text) {
            Ok(a) => return Ok(Some(a)),
            Err(reason) => reason,
        };
        let Some(second) = self.ask(prompt::repair(system, &reason, &first.text), user)? else {
            return Ok(None);
        };
        if cut_off(&second) {
            return Ok(None);
        }
        parse::parse(&second.text).map(Some).map_err(|again| {
            Failure::InvalidOutput(format!("{reason}; after the repair pass: {again}"))
        })
    }

    /// The analysis of one piece of transcript into `out`, halved and
    /// analysed in two (in order) when the answer was cut off.
    fn map(
        &mut self,
        text: &str,
        label: (usize, usize),
        depth: u32,
        out: &mut Vec<Analysis>,
    ) -> Result<(), Failure> {
        let user = if label.1 == 1 && depth == 0 {
            prompt::whole(text)
        } else {
            prompt::part(text, label.0, label.1)
        };
        if let Some(analysis) = self.attempt(prompt::SYSTEM_PROMPT, &user)? {
            out.push(analysis);
            return Ok(());
        }
        let halves = if depth < MAX_SPLITS {
            reduce::halve(text)
        } else {
            None
        };
        let Some((head, tail)) = halves else {
            return Err(Failure::InvalidOutput(format!(
                "the answer was cut off by the model's output budget (or the prompt did not fit its context) and the part could not be split further after {depth} splits; a smaller [meetings.analysis] chunk_chars or a larger [engines.llm] context_length would fit"
            )));
        };
        self.map(&head, label, depth + 1, out)?;
        self.map(&tail, label, depth + 1, out)
    }

    /// The summaries of the parts combined into one by the model, at
    /// most `limit` characters of summaries per call. A call the budget
    /// cut off (or whose prompt did not fit) lowers the limit under that
    /// group and combines it in smaller groups instead, so no call over
    /// that many characters is tried again.
    fn reduce_summaries(&mut self, mut summaries: Vec<String>) -> Result<String, Failure> {
        let mut limit = self.chunk_chars;
        while summaries.len() > 1 {
            let mut next = Vec::new();
            for group in reduce::groups(&summaries, limit) {
                if group.len() == 1 {
                    next.extend(group);
                    continue;
                }
                next.extend(self.merge(group, &mut limit)?);
            }
            summaries = next;
        }
        Ok(summaries.pop().unwrap_or_default())
    }

    /// `group` (two or more summaries) combined by the model into one,
    /// or, when that call did not fit, into as few as it could combine:
    /// the group halved and each half merged on its own (a half of one
    /// passes through), in order. A pair that does not fit fails the
    /// run naming why.
    fn merge(&mut self, group: Vec<String>, limit: &mut usize) -> Result<Vec<String>, Failure> {
        if let Some(merged) = self.attempt(prompt::MERGE_SYSTEM_PROMPT, &prompt::merge(&group))? {
            return Ok(vec![merged.summary]);
        }
        if group.len() <= 2 {
            return Err(Failure::InvalidOutput(
                "two summaries could not be combined: the combined summary was cut off by the model's output budget (or the merge prompt did not fit its context)".into(),
            ));
        }
        let chars: usize = group.iter().map(|s| s.chars().count()).sum();
        *limit = (*limit).min(chars / 2);
        let mut out = Vec::new();
        let (head, tail) = group.split_at(group.len() / 2);
        for half in [head, tail] {
            if half.len() == 1 {
                out.push(half[0].clone());
            } else {
                out.extend(self.merge(half.to_vec(), limit)?);
            }
        }
        Ok(out)
    }
}

/// Runs the analysis of `transcript` through `provider` under `settings`.
pub fn run(
    transcript: &str,
    provider: &dyn LlmProvider,
    settings: &Settings,
    cancel: Option<&AtomicBool>,
) -> Result<Outcome, Failure> {
    if transcript.trim().is_empty() {
        return Err(Failure::EmptyTranscript);
    }
    let mut run = Run {
        provider,
        deadline: Instant::now() + settings.timeout,
        cancel,
        calls: 0,
        chunk_chars: settings.chunk_chars.max(200),
    };
    let chunks = chunk(transcript, settings.chunk_chars);
    let total = chunks.len();
    let mut parts = Vec::with_capacity(total);
    for (i, piece) in chunks.iter().enumerate() {
        run.map(piece, (i + 1, total), 0, &mut parts)?;
    }
    let analysis = if parts.len() == 1 {
        parts.pop().unwrap_or_default()
    } else {
        let (decisions, action_items) = reduce::combine(&parts);
        let summaries = parts.iter().map(|p| p.summary.clone()).collect();
        Analysis {
            summary: run.reduce_summaries(summaries)?,
            decisions,
            action_items,
        }
    };
    Ok(Outcome {
        analysis,
        model: provider.model(),
        calls: run.calls,
        parts: parts.len().max(1) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::methods::meetings::SegmentSource;

    #[test]
    fn lines_carry_the_speaker_or_the_side_and_chunks_cut_on_lines() {
        let segments = vec![
            Segment {
                index: 0,
                start_ms: 0,
                end_ms: 1,
                text: "um hi".into(),
                speaker: Some("Mara".into()),
                speaker_id: Some("speaker_00".into()),
                speaker_confidence: Some(1.0),
                source_type: SegmentSource::System,
                words: Vec::new(),
                gap_before_ms: None,
                polished_text: Some("Hi.".into()),
            },
            Segment {
                index: 1,
                start_ms: 2,
                end_ms: 3,
                text: "hello".into(),
                speaker: None,
                speaker_id: None,
                speaker_confidence: None,
                source_type: SegmentSource::Microphone,
                words: Vec::new(),
                gap_before_ms: None,
                polished_text: None,
            },
        ];
        assert_eq!(transcript_lines(&segments), "[Mara] Hi.\n[you] hello");
        let long: Vec<String> = (0..10)
            .map(|i| format!("[you] line {i} {}", "x".repeat(60)))
            .collect();
        let parts = chunk(&long.join("\n"), 200);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.chars().count() <= 200));
        assert_eq!(parts.iter().flat_map(|p| p.lines()).count(), 10);
        assert_eq!(chunk("", 200).len(), 0);
        assert_eq!(Failure::EmptyTranscript.code(), "emptyTranscript");
    }
}
