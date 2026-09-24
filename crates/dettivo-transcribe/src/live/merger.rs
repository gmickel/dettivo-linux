//! Folding one source's window results into segments. The windows that
//! are not yet final go through the offline merger (`crate::merger`) as
//! consecutive chunks, so overlapping windows reconcile by timestamp the
//! way import chunks do; whatever ends more than the boundary merge gap
//! before the newest audio becomes final and gets its sequence number,
//! the rest is the provisional tail that the next window replaces whole.
//! A gap the capture recorded rides on the first segment after it.

use dettivo_engine_proto::Segment;

use crate::live::{LiveSegment, LiveSettings, Source, Window};
use crate::merger::{ChunkResult, merge};

/// What one window changed: finals to append, and the whole provisional
/// tail as it stands now (it replaces the previous tail).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Update {
    /// Segments that became final, in order.
    pub finals: Vec<LiveSegment>,
    /// The provisional tail after this window.
    pub provisional: Vec<LiveSegment>,
}

impl Update {
    /// True when nothing changed.
    pub fn is_empty(&self) -> bool {
        self.finals.is_empty() && self.provisional.is_empty()
    }
}

/// The merger of one source.
pub struct Merger {
    source: Source,
    boundary_gap_ms: u64,
    pending: Vec<ChunkResult>,
    finals: Vec<LiveSegment>,
    provisional: Vec<LiveSegment>,
    last_final_end_ms: u64,
    next_sequence: u64,
    /// A gap waiting for the first segment that starts at or after it:
    /// `(at_ms, gap_ms)`.
    gap: Option<(u64, u64)>,
}

impl Merger {
    /// A merger for `source` whose numbering starts at `next_sequence`.
    pub fn new(source: Source, settings: &LiveSettings, next_sequence: u64) -> Self {
        Self {
            source,
            boundary_gap_ms: settings.boundary_merge_gap_ms,
            pending: Vec::new(),
            finals: Vec::new(),
            provisional: Vec::new(),
            last_final_end_ms: 0,
            next_sequence,
            gap: None,
        }
    }

    /// Which side.
    pub fn source(&self) -> Source {
        self.source
    }

    /// The final segments so far.
    pub fn finals(&self) -> &[LiveSegment] {
        &self.finals
    }

    /// The provisional tail.
    pub fn provisional(&self) -> &[LiveSegment] {
        &self.provisional
    }

    /// The next sequence number a final gets.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// The end of the last final segment.
    pub fn last_end_ms(&self) -> u64 {
        self.finals.last().map(|s| s.end_ms).unwrap_or(0)
    }

    /// Records that `gap_ms` of capture is missing before `at_ms`; the
    /// first segment starting there or later carries it. A new take that
    /// starts before the last final ended (a clock that ran ahead) never
    /// hides the take's segments behind that end.
    pub fn note_gap(&mut self, at_ms: u64, gap_ms: u64) {
        self.gap = Some((at_ms, gap_ms));
        self.last_final_end_ms = self.last_final_end_ms.min(at_ms);
    }

    fn segment(&self, s: &Segment, id: String, provisional: bool) -> LiveSegment {
        LiveSegment {
            source: self.source,
            id,
            provisional,
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text.clone(),
            words: s.words.clone(),
            gap_before_ms: None,
        }
    }

    /// Recomputes the pending windows and settles everything that ends
    /// before `boundary_ms`.
    fn settle(&mut self, boundary_ms: u64) -> Update {
        let merged = merge(&self.pending);
        let mut update = Update::default();
        let mut provisional = Vec::new();
        for s in merged.iter().filter(|s| !s.text.trim().is_empty()) {
            if s.end_ms <= self.last_final_end_ms {
                continue;
            }
            if s.end_ms <= boundary_ms {
                let id = format!("{}-{}", self.source.as_str(), self.next_sequence);
                self.next_sequence += 1;
                let mut seg = self.segment(s, id, false);
                if let Some((at, gap)) = self.gap
                    && seg.start_ms >= at
                {
                    seg.gap_before_ms = Some(gap);
                    self.gap = None;
                }
                self.last_final_end_ms = seg.end_ms;
                self.finals.push(seg.clone());
                update.finals.push(seg);
            } else {
                let id = format!("{}-p{}", self.source.as_str(), provisional.len() + 1);
                let mut seg = self.segment(s, id, true);
                if let Some((at, gap)) = self.gap
                    && seg.start_ms >= at
                    && provisional.is_empty()
                {
                    seg.gap_before_ms = Some(gap);
                }
                provisional.push(seg);
            }
        }
        // Windows the finals already cover are dropped, all but the last
        // one, which anchors the seam with the next window.
        let covered = self
            .pending
            .iter()
            .filter(|w| w.end_ms <= self.last_final_end_ms)
            .count();
        if covered > 1 {
            self.pending.drain(..covered - 1);
        }
        self.provisional = provisional.clone();
        update.provisional = provisional;
        update
    }

    /// Folds one window's segments (timed relative to the window) in.
    pub fn apply(&mut self, window: &Window, segments: Vec<Segment>) -> Update {
        self.pending.push(ChunkResult {
            start_ms: window.start_ms,
            end_ms: window.end_ms,
            segments,
        });
        self.settle(window.end_ms.saturating_sub(self.boundary_gap_ms))
    }

    /// A span with no engine call (silence, a skip) ending at `end_ms`:
    /// the tail behind the boundary hardens; an empty update when nothing
    /// changed.
    pub fn advance(&mut self, end_ms: u64) -> Update {
        if self.pending.is_empty() {
            return Update::default();
        }
        let before = self.provisional.clone();
        let update = self.settle(end_ms.saturating_sub(self.boundary_gap_ms));
        if update.finals.is_empty() && update.provisional == before {
            return Update::default();
        }
        update
    }

    /// Stop: every provisional segment becomes final.
    pub fn flush(&mut self) -> Update {
        if self.pending.is_empty() {
            return Update::default();
        }
        let mut update = self.settle(u64::MAX);
        update.provisional.clear();
        self.pending.clear();
        update
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(start: u64, end: u64) -> Window {
        Window {
            source: Source::You,
            start_ms: start,
            end_ms: end,
            samples: Vec::new(),
        }
    }

    fn seg(start: u64, end: u64, text: &str) -> Segment {
        Segment {
            start_ms: start,
            end_ms: end,
            text: text.into(),
            words: Vec::new(),
        }
    }

    fn texts(segments: &[LiveSegment]) -> Vec<(String, u64, u64, bool)> {
        segments
            .iter()
            .map(|s| (s.id.clone(), s.start_ms, s.end_ms, s.provisional))
            .collect()
    }

    #[test]
    fn provisional_tails_are_replaced_and_finalise_after_the_boundary_gap() {
        let mut m = Merger::new(Source::You, &LiveSettings::default(), 0);
        // Window 1: 0..900, one segment still inside the 1200 ms gap.
        let u = m.apply(&win(0, 900), vec![seg(100, 800, "ask not what")]);
        assert!(u.finals.is_empty());
        assert_eq!(
            texts(&u.provisional),
            [("you-p1".to_string(), 100, 800, true)]
        );
        // Window 2: 450..1800 repeats the tail and adds words; the tail is
        // replaced whole, nothing older than 600 ms is final yet.
        let u = m.apply(
            &win(450, 1800),
            vec![seg(0, 350, "what"), seg(400, 1300, "your country can do")],
        );
        assert!(u.finals.is_empty());
        assert_eq!(u.provisional.len(), 2, "{:?}", u.provisional);
        assert_eq!(u.provisional[0].text, "ask not what");
        assert_eq!(u.provisional[1].text, "your country can do");
        // Window 3: 1350..2700; the boundary is 1500, so the first
        // segment hardens with its sequence number and the rest stays
        // provisional, renumbered from p1.
        let u = m.apply(&win(1350, 2700), vec![seg(600, 1300, "for you")]);
        assert_eq!(texts(&u.finals), [("you-0".to_string(), 100, 800, false)]);
        assert_eq!(
            texts(&u.provisional),
            [
                ("you-p1".to_string(), 850, 1750, true),
                ("you-p2".to_string(), 1950, 2650, true)
            ]
        );
        assert_eq!(m.finals().len(), 1);
        assert_eq!(m.next_sequence(), 1);
        assert_eq!(m.last_end_ms(), 800);
        // Silence advances the boundary: the tail hardens.
        let u = m.advance(4000);
        assert_eq!(
            texts(&u.finals),
            [
                ("you-1".to_string(), 850, 1750, false),
                ("you-2".to_string(), 1950, 2650, false)
            ]
        );
        assert!(u.provisional.is_empty());
        assert!(m.advance(5000).is_empty(), "nothing changes twice");
        assert!(m.flush().is_empty() || m.provisional().is_empty());
    }

    #[test]
    fn a_gap_rides_on_the_first_segment_after_it_and_flush_finalises() {
        let mut m = Merger::new(Source::Remote, &LiveSettings::default(), 7);
        m.apply(&win(0, 900), vec![seg(0, 800, "before")]);
        m.note_gap(5000, 4100);
        let u = m.apply(&win(5000, 5900), vec![seg(100, 700, "after")]);
        assert_eq!(u.finals[0].id, "remote-7");
        assert_eq!(u.finals[0].gap_before_ms, None);
        assert_eq!(u.provisional[0].gap_before_ms, Some(4100));
        let u = m.flush();
        assert_eq!(
            texts(&u.finals),
            [("remote-8".to_string(), 5100, 5700, false)]
        );
        assert_eq!(u.finals[0].gap_before_ms, Some(4100));
        assert!(m.provisional().is_empty());
        assert_eq!(m.next_sequence(), 9);
    }
}
