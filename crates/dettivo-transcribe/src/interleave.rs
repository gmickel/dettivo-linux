//! Two sources on one timeline. Segments sort by their start on the
//! meeting clock (the other side first on a tie), and the macOS
//! cross-source rule (`LiveMeetingSegmentMerger.shouldSuppress`) drops a
//! microphone segment that is a filler or four characters or less while
//! it overlaps, within the padding, a neighbouring remote segment at least
//! twice as long and eighteen characters or more: the microphone heard
//! the speakers, and the remote track already has the words.

use crate::live::{LiveSegment, Source};

/// The macOS filler list a microphone segment is judged against.
pub const FILLERS: &[&str] = &[
    "thank you",
    "thanks",
    "okay",
    "ok",
    "yeah",
    "yes",
    "yo",
    "right",
    "all right",
    "gracias",
    "sorry",
    "tip top",
    "tipptopp",
    "wow",
];

/// The macOS normalisation: lower case, trimmed, trailing punctuation and
/// symbols removed, whitespace collapsed.
pub fn normalised(text: &str) -> String {
    let lower = text.to_lowercase();
    let trimmed = lower
        .trim()
        .trim_end_matches(|c: char| !c.is_alphanumeric() && !c.is_whitespace());
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_filler(text: &str) -> bool {
    FILLERS.contains(&text) || text.chars().count() <= 4
}

/// True when `candidate` (a microphone segment) is the echo of `other`.
fn suppressed(candidate: &LiveSegment, other: &LiveSegment, padding_ms: u64) -> bool {
    if candidate.source == other.source || candidate.source != Source::You {
        return false;
    }
    let mine = normalised(&candidate.text);
    let theirs = normalised(&other.text);
    if !is_filler(&mine) {
        return false;
    }
    if theirs.chars().count() < (mine.chars().count() * 2).max(18) {
        return false;
    }
    candidate.start_ms <= other.end_ms + padding_ms
        && other.start_ms <= candidate.end_ms + padding_ms
}

/// Orders `segments` on the meeting clock and applies the cross-source
/// suppression with `padding_ms`.
pub fn interleave(mut segments: Vec<LiveSegment>, padding_ms: u64) -> Vec<LiveSegment> {
    segments.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then_with(|| (a.source == Source::You).cmp(&(b.source == Source::You)))
            .then_with(|| a.end_ms.cmp(&b.end_ms))
    });
    if segments.len() < 2 {
        return segments;
    }
    let mut kept = Vec::with_capacity(segments.len());
    for (i, candidate) in segments.iter().enumerate() {
        let previous = i.checked_sub(1).map(|p| &segments[p]);
        let next = segments.get(i + 1);
        let echo = previous.is_some_and(|p| suppressed(candidate, p, padding_ms))
            || next.is_some_and(|n| suppressed(candidate, n, padding_ms));
        if !echo {
            kept.push(candidate.clone());
        }
    }
    kept
}

/// The transcript text: every segment's text in order, one space apart.
pub fn text_of(segments: &[LiveSegment]) -> String {
    segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(source: Source, start: u64, end: u64, text: &str) -> LiveSegment {
        LiveSegment {
            source,
            id: String::new(),
            provisional: false,
            start_ms: start,
            end_ms: end,
            text: text.into(),
            words: Vec::new(),
            gap_before_ms: None,
        }
    }

    #[test]
    fn segments_interleave_by_time_with_the_remote_side_first_on_a_tie() {
        let out = interleave(
            vec![
                seg(Source::You, 2000, 3000, "I agree with that plan"),
                seg(
                    Source::Remote,
                    0,
                    1900,
                    "we should ship the merger this week",
                ),
                seg(Source::You, 0, 800, "morning everyone here"),
            ],
            800,
        );
        let order: Vec<(Source, u64)> = out.iter().map(|s| (s.source, s.start_ms)).collect();
        assert_eq!(
            order,
            [(Source::Remote, 0), (Source::You, 0), (Source::You, 2000)]
        );
        assert_eq!(
            text_of(&out),
            "we should ship the merger this week morning everyone here I agree with that plan"
        );
    }

    #[test]
    fn a_microphone_filler_inside_the_padding_of_a_remote_span_is_echo() {
        // "Yeah." on the microphone 600 ms after a long remote sentence
        // ended: the speakers, not the person.
        let out = interleave(
            vec![
                seg(
                    Source::Remote,
                    0,
                    4000,
                    "so the finalisation reads every take",
                ),
                seg(Source::You, 4600, 5000, "Yeah."),
                seg(Source::You, 6000, 8000, "and the system track as well"),
            ],
            800,
        );
        let texts: Vec<&str> = out.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "so the finalisation reads every take",
                "and the system track as well"
            ]
        );
        // The same filler past the padding survives, and a remote filler
        // is never suppressed.
        let out = interleave(
            vec![
                seg(
                    Source::Remote,
                    0,
                    4000,
                    "so the finalisation reads every take",
                ),
                seg(Source::You, 4900, 5300, "Yeah."),
                seg(Source::Remote, 5400, 5600, "ok"),
            ],
            800,
        );
        assert_eq!(out.len(), 3);
        // Real microphone words inside the span are kept.
        let out = interleave(
            vec![
                seg(
                    Source::Remote,
                    0,
                    4000,
                    "so the finalisation reads every take",
                ),
                seg(Source::You, 1000, 2500, "wait, which take exactly?"),
            ],
            800,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(normalised("  Tip-Top!! "), "tip-top");
        assert_eq!(normalised("All right..."), "all right");
    }
}
