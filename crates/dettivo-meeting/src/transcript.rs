//! The transcript so far (`meetings.segments`, ADR 0071). The live tail
//! of a meeting is registered here when it starts and stays readable
//! through the stop and the finalisation, until the settled row is
//! stored; a read takes the tail's lock only to copy the finals after the
//! cursor and the provisional tail, so it never waits on the engine and
//! never writes. Without a tail the stored row answers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use dettivo_proto::methods::meetings::Segment;
use dettivo_proto::methods::meetings_segments::{Cursor, TranscriptSegment, TranscriptSource};
use dettivo_transcribe::live::LiveSegment;

use crate::live::Tail;
use crate::machine::Session;

/// The live tails by meeting id.
#[derive(Clone, Default)]
pub(crate) struct LiveTails(Arc<Mutex<HashMap<String, Arc<Mutex<Tail>>>>>);

impl LiveTails {
    fn map(&self) -> MutexGuard<'_, HashMap<String, Arc<Mutex<Tail>>>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The tail of `meeting_id`, registered on first use.
    pub(crate) fn tail(&self, meeting_id: &str) -> Arc<Mutex<Tail>> {
        self.map()
            .entry(meeting_id.to_string())
            .or_default()
            .clone()
    }

    /// Forgets the tail once the stored row replaces it.
    pub(crate) fn remove(&self, meeting_id: &str) {
        self.map().remove(meeting_id);
    }

    fn get(&self, meeting_id: &str) -> Option<Arc<Mutex<Tail>>> {
        self.map().get(meeting_id).cloned()
    }
}

/// One read of a meeting's transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    /// Which transcript answered.
    pub transcript: TranscriptSource,
    /// The cursor to pass back.
    pub cursor: String,
    /// True when `since` did not belong to this transcript; `segments`
    /// is then the whole transcript.
    pub reset: bool,
    /// The finals after `since`.
    pub segments: Vec<TranscriptSegment>,
    /// The provisional tail.
    pub provisional: Vec<TranscriptSegment>,
}

fn live_segment(segment: &LiveSegment, index: usize) -> TranscriptSegment {
    let mut out = TranscriptSegment::stored(&segment.contract(index as u32));
    out.segment_id = segment.id.clone();
    out.provisional = segment.provisional;
    out
}

/// Where a read starts in a transcript of `total` finals, and whether
/// `since` forces a reset.
fn start(since: Option<Cursor>, transcript: TranscriptSource, total: usize) -> (usize, bool) {
    match since {
        None => (0, false),
        Some(c) if c.transcript == transcript && c.finals <= total => (c.finals, false),
        Some(_) => (0, true),
    }
}

impl Session {
    /// The transcript of `meeting_id` after `since`: the live tail while
    /// one is registered, else what `stored` answers (the row's segments;
    /// its error passes through).
    pub fn read_segments<E>(
        &self,
        meeting_id: &str,
        since: Option<Cursor>,
        stored: impl FnOnce() -> Result<Vec<Segment>, E>,
    ) -> Result<Read, E> {
        if let Some(tail) = self.tails.get(meeting_id) {
            let (first, reset, finals, provisional, total) = {
                let t = tail.lock().unwrap_or_else(|p| p.into_inner());
                let (first, reset) = start(since, TranscriptSource::Live, t.finals.len());
                (
                    first,
                    reset,
                    t.finals[first..].to_vec(),
                    t.provisional.clone(),
                    t.finals.len(),
                )
            };
            return Ok(Read {
                transcript: TranscriptSource::Live,
                cursor: Cursor {
                    transcript: TranscriptSource::Live,
                    finals: total,
                }
                .render(),
                reset,
                segments: finals
                    .iter()
                    .enumerate()
                    .map(|(i, s)| live_segment(s, first + i))
                    .collect(),
                provisional: provisional
                    .iter()
                    .enumerate()
                    .map(|(i, s)| live_segment(s, total + i))
                    .collect(),
            });
        }
        let segments = stored()?;
        let (first, reset) = start(since, TranscriptSource::Stored, segments.len());
        Ok(Read {
            transcript: TranscriptSource::Stored,
            cursor: Cursor {
                transcript: TranscriptSource::Stored,
                finals: segments.len(),
            }
            .render(),
            reset,
            segments: segments[first..]
                .iter()
                .map(TranscriptSegment::stored)
                .collect(),
            provisional: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_storage::meetings::MeetingArtifacts;
    use dettivo_transcribe::live::Source;

    struct Quiet;
    impl crate::Publisher for Quiet {
        fn state(&self, _: &crate::StateChange) {}
        fn level(&self, _: crate::Level) {}
        fn segment(&self, _: &str, _: &LiveSegment) {}
        fn progress(&self, _: &str, _: &str, _: crate::FinalizeProgress) {}
    }
    impl crate::Archive for Quiet {
        fn started(&self, _: &dettivo_storage::meetings::MeetingRow) -> Result<(), String> {
            Ok(())
        }
        fn updated(&self, _: &dettivo_storage::meetings::MeetingRow) -> Result<(), String> {
            Ok(())
        }
    }

    fn segment(source: Source, n: u64, provisional: bool) -> LiveSegment {
        LiveSegment {
            source,
            id: if provisional {
                format!("{}-p{}", source.as_str(), n + 1)
            } else {
                format!("{}-{n}", source.as_str())
            },
            provisional,
            start_ms: n * 3000,
            end_ms: n * 3000 + 2800,
            text: format!("segment {n} of the meeting with a few words in it"),
            words: (0..8)
                .map(|w| dettivo_engine_proto::Word {
                    start_ms: n * 3000 + w * 350,
                    end_ms: n * 3000 + w * 350 + 300,
                    text: format!("w{w}"),
                    confidence: 0.9,
                })
                .collect(),
            gap_before_ms: None,
        }
    }

    /// R1: a two-hour tail (a final every three seconds on each side,
    /// eight aligned words each) reads whole in well under 100 ms, the
    /// provisional tail flagged after the finals.
    #[test]
    fn a_two_hour_tail_reads_whole_and_in_order_within_100_ms() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(
            MeetingArtifacts::new(dir.path().to_path_buf()),
            Arc::new(Quiet),
            Arc::new(Quiet),
        );
        let tail = session.tails.tail("m");
        {
            let mut t = tail.lock().unwrap();
            for n in 0..2400 {
                t.finals.push(segment(Source::Remote, n, false));
                t.finals.push(segment(Source::You, n, false));
            }
            t.provisional = vec![segment(Source::You, 0, true)];
        }
        let started = std::time::Instant::now();
        let read = session
            .read_segments("m", None, || -> Result<_, ()> { unreachable!() })
            .unwrap();
        let took = started.elapsed();
        assert!(took < std::time::Duration::from_millis(100), "{took:?}");
        assert_eq!(read.transcript, TranscriptSource::Live);
        assert_eq!(read.cursor, "live:4800");
        assert_eq!(read.segments.len(), 4800);
        assert!(
            read.segments
                .iter()
                .enumerate()
                .all(|(i, s)| s.index as usize == i)
        );
        assert_eq!(read.segments[0].segment_id, "remote-0");
        assert_eq!(read.segments[1].source, "you");
        assert_eq!(read.provisional[0].segment_id, "you-p1");
        assert!(read.provisional[0].provisional && read.provisional[0].index == 4800);
    }
}
