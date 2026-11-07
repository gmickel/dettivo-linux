//! Dictation's rounds while the key is held (ADR 0102). An offline model
//! re-decodes the take so far at a fixed cadence; once the take is long,
//! the words well behind the newest audio are committed, and each later
//! round decodes only from a left context before the commit point. At the
//! release only the uncommitted tail is decoded, and its words are
//! stitched to the last round's by word timestamps: a word belongs to the
//! side its centre falls on. When only silence followed the last round,
//! that round's words are the answer and nothing is decoded.
//!
//! All times are milliseconds from the start of the take. This module
//! holds no audio and calls no engine: the session and the QA bench feed
//! it the words their decodes returned.

use dettivo_engine_proto::Word;

/// The windows the rounds and the release decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows {
    /// Between the starts of two rounds.
    pub cadence_ms: u64,
    /// No word is committed before a round covers this much of the take.
    pub commit_after_ms: u64,
    /// A word is committed once it ends this far behind the round's end.
    pub horizon_ms: u64,
    /// Audio before the commit point each round decodes again, as context.
    pub context_ms: u64,
    /// The release decodes about this much of the take's end.
    pub tail_ms: u64,
    /// And this much before it, so the cut word has context.
    pub overlap_ms: u64,
}

/// The windows dictation runs with (ADR 0102 gives the measurements).
pub const WINDOWS: Windows = Windows {
    cadence_ms: 1_500,
    commit_after_ms: 60_000,
    horizon_ms: 20_000,
    context_ms: 20_000,
    tail_ms: 4_000,
    overlap_ms: 2_000,
};

/// What the release decodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Release {
    /// Only silence followed the last round: its words are the take's.
    Reuse(Vec<Word>),
    /// Decode `from_ms` to the end and [`Rounds::stitch`] at `cut_ms`.
    Tail {
        /// Where the release decode starts.
        from_ms: u64,
        /// Words centred before it come from the rounds, the rest from the
        /// release decode.
        cut_ms: u64,
    },
    /// No round is recent enough: the whole take is decoded as before.
    Whole,
}

/// The last round's words from the commit point on, and where it ended.
#[derive(Debug, Clone)]
struct Round {
    until_ms: u64,
    words: Vec<Word>,
}

/// One take's committed words and its last round.
#[derive(Debug, Clone, Default)]
pub struct Rounds {
    committed: Vec<Word>,
    /// Words centred before this point are committed.
    commit_ms: u64,
    last: Option<Round>,
}

fn centre(w: &Word) -> u64 {
    (w.start_ms + w.end_ms) / 2
}

/// A cut between `a` and `b`: the middle of the pause between them, or of
/// their centres when their times overlap; always past `a`'s centre.
fn between(a: &Word, b: &Word) -> u64 {
    let cut = if a.end_ms <= b.start_ms {
        (a.end_ms + b.start_ms) / 2
    } else {
        (centre(a) + centre(b)).div_ceil(2)
    };
    cut.max(centre(a) + 1)
}

impl Rounds {
    /// The span the next round decodes once `captured_ms` of the take
    /// arrived: from the context before the commit point to the newest audio.
    pub fn window(&self, w: &Windows, captured_ms: u64) -> (u64, u64) {
        (self.commit_ms.saturating_sub(w.context_ms), captured_ms)
    }

    /// True once a round was absorbed.
    pub fn any(&self) -> bool {
        self.last.is_some()
    }

    /// The end of the last round absorbed.
    pub fn until_ms(&self) -> Option<u64> {
        self.last.as_ref().map(|r| r.until_ms)
    }

    /// Words committed so far.
    pub fn committed(&self) -> usize {
        self.committed.len()
    }

    /// Takes a round that decoded the take up to `until_ms`; `words` carry
    /// times from the take's start. Words centred before the commit point
    /// were context. Past [`Windows::commit_after_ms`], the words that end
    /// [`Windows::horizon_ms`] behind `until_ms` are committed.
    pub fn absorb(&mut self, w: &Windows, until_ms: u64, words: Vec<Word>) {
        let mut pending: Vec<Word> = words
            .into_iter()
            .filter(|x| centre(x) >= self.commit_ms)
            .collect();
        if until_ms >= w.commit_after_ms {
            let limit = until_ms.saturating_sub(w.horizon_ms);
            let n = pending.iter().take_while(|x| x.end_ms <= limit).count();
            if n > 0 {
                let last = &pending[n - 1];
                self.commit_ms = match pending.get(n) {
                    Some(next) => between(last, next),
                    None => last.end_ms.max(centre(last) + 1),
                };
                self.committed.extend(pending.drain(..n));
            }
        }
        self.last = Some(Round {
            until_ms,
            words: pending,
        });
    }

    /// What the release decodes for a take of `end_ms`. `voiced_until_ms`
    /// is where the take's last voiced audio ends, when the speech check
    /// knows; audio after the last round that holds no voice leaves that
    /// round's words as the take's.
    pub fn release(&self, w: &Windows, end_ms: u64, voiced_until_ms: Option<u64>) -> Release {
        let Some(round) = &self.last else {
            return Release::Whole;
        };
        // A round this old could not keep up with the take.
        if round.until_ms + w.horizon_ms < end_ms {
            return Release::Whole;
        }
        if voiced_until_ms.is_some_and(|v| v <= round.until_ms) {
            let mut words = self.committed.clone();
            words.extend(round.words.iter().cloned());
            return Release::Reuse(words);
        }
        // The round's last words had no audio after them: the cut stays an
        // overlap behind its end, and as near the tail's start as a pause
        // between words allows.
        let latest = round.until_ms.saturating_sub(w.overlap_ms);
        let target = end_ms.saturating_sub(w.tail_ms);
        let words = &round.words;
        let mut cuts = vec![(self.commit_ms, 0u64)];
        for pair in words.windows(2) {
            let gap = pair[1].start_ms.saturating_sub(pair[0].end_ms);
            cuts.push((between(&pair[0], &pair[1]), gap));
        }
        if let Some(last) = words.last() {
            cuts.push((last.end_ms.max(centre(last) + 1), u64::MAX));
        }
        let usable = cuts
            .into_iter()
            .filter(|(c, _)| *c >= self.commit_ms && *c <= latest);
        let near = |c: u64| c.abs_diff(target) <= w.overlap_ms;
        let Some((cut_ms, _)) = usable.fold(None, |best: Option<(u64, u64)>, (c, gap)| {
            let better = match best {
                None => true,
                // A pause inside the overlap of the target wins by its
                // length; otherwise the cut nearest the target.
                Some((b, bgap)) => match (near(c), near(b)) {
                    (true, true) => {
                        gap > bgap || (gap == bgap && c.abs_diff(target) < b.abs_diff(target))
                    }
                    (true, false) => true,
                    (false, true) => false,
                    (false, false) => c.abs_diff(target) < b.abs_diff(target),
                },
            };
            if better { Some((c, gap)) } else { best }
        }) else {
            return Release::Whole;
        };
        Release::Tail {
            from_ms: cut_ms.saturating_sub(w.overlap_ms),
            cut_ms,
        }
    }

    /// The take's words: the committed ones, the last round's centred
    /// before `cut_ms` and the release decode's centred at or after it.
    /// `tail` carries times from the take's start.
    pub fn stitch(&self, cut_ms: u64, tail: Vec<Word>) -> Vec<Word> {
        let mut words = self.committed.clone();
        if let Some(round) = &self.last {
            words.extend(round.words.iter().filter(|x| centre(x) < cut_ms).cloned());
        }
        words.extend(tail.into_iter().filter(|x| centre(x) >= cut_ms));
        words
    }
}

/// Words from a decode of a clip that starts `from_ms` into the take.
pub fn shifted(words: impl IntoIterator<Item = Word>, from_ms: u64) -> Vec<Word> {
    words
        .into_iter()
        .map(|mut x| {
            x.start_ms += from_ms;
            x.end_ms += from_ms;
            x
        })
        .collect()
}

/// The text of stitched words, joined as the engine joins stitched chunks.
pub fn text(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "rounds_tests.rs"]
mod tests;
