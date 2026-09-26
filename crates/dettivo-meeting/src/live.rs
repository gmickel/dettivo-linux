//! The live transcriber (ADR 0030): a thread of its own that receives both
//! tracks' samples from the capture worker, cuts them into windows at the
//! live tuning, sends each window through the engine (the remote track
//! first when both are due, one call at a time), folds the results into
//! per-source segments and publishes them as `meeting.segment` events,
//! provisional then final. Audio that piles up while the engine works is
//! skipped by the windower and journaled, so the live view stays current
//! and finalisation covers the rest.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_engine_proto::{RecognizeResult, Segment};
use dettivo_speech::{EngineError, RecognizeRequest, SttEngine};
use dettivo_transcribe::live::{Cut, LiveSegment, LiveSettings, Merger, Source, Windower};
use dettivo_transcribe::{Settings, filters};

use crate::journal::Journal;
use crate::{Publisher, Track};

/// The longest wait for one live window.
pub const WINDOW_TIMEOUT: Duration = Duration::from_secs(30);

/// What the worker tells the live thread.
pub(crate) enum Command {
    /// Samples of a track's current take.
    Pcm(Track, Vec<i16>),
    /// A new take of `track` began `origin_ms` into the meeting after
    /// `gap_ms` without capture.
    Restart {
        track: Track,
        origin_ms: u64,
        gap_ms: u64,
    },
    /// The capture stopped: the tails are transcribed and the thread ends.
    Flush,
}

/// The live segments as the checkpoint and `meeting.state` read them.
#[derive(Debug, Clone, Default)]
pub struct Tail {
    /// Final segments of both sources, in the order they became final.
    pub finals: Vec<LiveSegment>,
    /// The provisional tail per source.
    pub provisional: Vec<LiveSegment>,
    /// The next sequence number, over both sources.
    pub next_sequence: u64,
    /// Why the live path stopped, when it did.
    pub unavailable: Option<String>,
}

impl Tail {
    /// The end of the last final segment.
    pub fn last_end_ms(&self) -> u64 {
        self.finals.iter().map(|s| s.end_ms).max().unwrap_or(0)
    }
}

/// The worker's side of the live thread.
pub(crate) struct LiveHandle {
    tx: Sender<Command>,
    tail: Arc<Mutex<Tail>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LiveHandle {
    /// Sends one command; a thread that ended drops it.
    pub(crate) fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }

    /// A copy of the tail.
    pub(crate) fn tail(&self) -> Tail {
        self.tail.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Flushes and waits for the thread.
    pub(crate) fn finish(mut self) -> Tail {
        let _ = self.tx.send(Command::Flush);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.tail()
    }
}

/// What a source needs on the live thread.
struct Lane {
    windower: Windower,
    merger: Merger,
}

/// The live thread's state.
pub(crate) struct Live {
    meeting_id: String,
    engine: Arc<dyn SttEngine>,
    language: String,
    prompt: Option<String>,
    filler_filter: bool,
    publisher: Arc<dyn Publisher>,
    journal: Journal,
    tail: Arc<Mutex<Tail>>,
    you: Lane,
    remote: Lane,
    unavailable: bool,
}

/// Spawns the live thread for a meeting whose first takes begin at
/// `origins_ms` (microphone, system) on the meeting clock, the same
/// offsets the take manifests carry into the finalisation;
/// `next_sequence` continues a checkpoint's numbering. The thread folds
/// its segments into `tail`, which `meetings.segments` reads beside it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    meeting_id: &str,
    engine: Arc<dyn SttEngine>,
    settings: LiveSettings,
    transcribe: &Settings,
    language: &str,
    prompt: Option<String>,
    publisher: Arc<dyn Publisher>,
    journal: Journal,
    origins_ms: (u64, u64),
    next_sequence: u64,
    tail: Arc<Mutex<Tail>>,
) -> std::io::Result<LiveHandle> {
    let (tx, rx) = std::sync::mpsc::channel();
    tail.lock().unwrap_or_else(|p| p.into_inner()).next_sequence = next_sequence;
    let lane = |source: Source, origin_ms: u64| Lane {
        windower: Windower::new(source, settings.clone(), origin_ms),
        merger: Merger::new(source, &settings, next_sequence),
    };
    let mut live = Live {
        meeting_id: meeting_id.to_string(),
        engine,
        language: if language.is_empty() {
            "auto".into()
        } else {
            language.to_string()
        },
        prompt,
        filler_filter: transcribe.filler_filter,
        publisher,
        journal,
        tail: tail.clone(),
        you: lane(Source::You, origins_ms.0),
        remote: lane(Source::Remote, origins_ms.1),
        unavailable: false,
    };
    if !live.engine.capabilities().supports_timestamps {
        live.disable(format!(
            "provider {} returns no timestamps; the live path needs them",
            live.engine.provider()
        ));
    }
    let thread = std::thread::Builder::new()
        .name(format!(
            "meeting-live-{}",
            &meeting_id[..meeting_id.len().min(8)]
        ))
        .spawn(move || live.run(rx))?;
    Ok(LiveHandle {
        tx,
        tail,
        thread: Some(thread),
    })
}

/// The segments of one window's result, or the reason the engine cannot
/// serve the live path: text without segments means no timestamps.
pub fn window_segments(result: RecognizeResult, provider: &str) -> Result<Vec<Segment>, String> {
    if !result.text.trim().is_empty() && result.segments.is_empty() {
        return Err(format!(
            "provider {provider} returned text without timestamps; the live path needs them"
        ));
    }
    Ok(result.segments)
}

impl Live {
    fn lane(&mut self, track: Track) -> &mut Lane {
        match track {
            Track::Microphone => &mut self.you,
            Track::System => &mut self.remote,
        }
    }

    fn disable(&mut self, why: String) {
        if self.unavailable {
            return;
        }
        self.unavailable = true;
        tracing::warn!(reason = %why, "meeting: live transcription unavailable");
        self.journal
            .record("live_unavailable", None, None, Some(why.clone()));
        self.tail
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .unavailable = Some(why);
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            // Block for the first command, then drain what else arrived
            // so a slow engine call never starves the windowers.
            let first = match rx.recv() {
                Ok(c) => c,
                Err(_) => return,
            };
            let mut flush = self.handle(first);
            loop {
                match rx.try_recv() {
                    Ok(c) => flush |= self.handle(c),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        flush = true;
                        break;
                    }
                }
            }
            self.tick(flush);
            if flush {
                return;
            }
        }
    }

    /// Applies one command; true when it was the flush.
    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Pcm(track, samples) => {
                self.lane(track).windower.push(&samples);
                false
            }
            Command::Restart {
                track,
                origin_ms,
                gap_ms,
            } => {
                // A take boundary is hard: the old take's tail hardens
                // before the new take's windows arrive, and the first
                // segment of the new take carries the gap.
                let cuts = self.lane(track).windower.restart(origin_ms);
                self.process(track, cuts);
                let update = self.lane(track).merger.flush();
                self.publish(update);
                self.lane(track).merger.note_gap(origin_ms, gap_ms);
                false
            }
            Command::Flush => true,
        }
    }

    /// Cuts what is due on both lanes, the remote side first, and at the
    /// flush transcribes the tails and hardens every provisional segment.
    fn tick(&mut self, flush: bool) {
        for track in [Track::System, Track::Microphone] {
            let cuts = if flush {
                self.lane(track).windower.flush()
            } else {
                self.lane(track).windower.cut()
            };
            self.process(track, cuts);
        }
        if flush {
            for track in [Track::System, Track::Microphone] {
                let update = self.lane(track).merger.flush();
                self.publish(update);
            }
        }
    }

    fn process(&mut self, track: Track, cuts: Vec<Cut>) {
        for cut in cuts {
            match cut {
                Cut::Window(window) => {
                    if self.unavailable {
                        continue;
                    }
                    match self.recognize(&window.samples) {
                        Ok(segments) => {
                            let segments = self.filtered(segments, track);
                            let update = self.lane(track).merger.apply(&window, segments);
                            self.publish(update);
                        }
                        Err(LiveError::NoTimestamps(why)) => self.disable(why),
                        Err(LiveError::Engine(why)) => {
                            tracing::warn!(reason = %why, "meeting: live window failed");
                            self.journal.record(
                                "live_error",
                                Some(track),
                                None,
                                Some(format!("{}-{} ms: {why}", window.start_ms, window.end_ms)),
                            );
                            let update = self.lane(track).merger.advance(window.end_ms);
                            self.publish(update);
                        }
                    }
                }
                Cut::Silent { end_ms, .. } => {
                    let update = self.lane(track).merger.advance(end_ms);
                    self.publish(update);
                }
                Cut::Skipped { start_ms, end_ms } => {
                    self.journal.record(
                        "live_skip",
                        Some(track),
                        None,
                        Some(format!(
                            "{start_ms}-{end_ms} ms skipped; the engine lagged more than two ticks"
                        )),
                    );
                    let lane = self.lane(track);
                    lane.merger.note_gap(end_ms, end_ms - start_ms);
                    let update = lane.merger.advance(end_ms);
                    self.publish(update);
                }
            }
        }
    }

    fn filtered(&self, segments: Vec<Segment>, track: Track) -> Vec<Segment> {
        if !self.filler_filter {
            return segments;
        }
        segments
            .into_iter()
            .filter(|s| {
                !filters::should_drop(
                    &s.text,
                    s.end_ms.saturating_sub(s.start_ms),
                    track == Track::System,
                )
            })
            .collect()
    }

    fn recognize(&self, pcm: &[i16]) -> Result<Vec<Segment>, LiveError> {
        let make = || RecognizeRequest {
            pcm: pcm.to_vec(),
            language: self.language.clone(),
            prompt: self.prompt.clone(),
            timestamps: true,
        };
        let result = match self.engine.recognize(make(), WINDOW_TIMEOUT) {
            Err(EngineError::Crashed(_)) => {
                tracing::warn!("engine crashed on a live window; retrying once");
                self.engine.recognize(make(), WINDOW_TIMEOUT)
            }
            other => other,
        }
        .map_err(|e| LiveError::Engine(dettivo_transcribe::job::redact(&e)))?;
        window_segments(result, self.engine.provider()).map_err(LiveError::NoTimestamps)
    }

    /// Publishes an update and folds it into the tail.
    fn publish(&mut self, update: dettivo_transcribe::live::Update) {
        if update.is_empty() && self.you.merger.provisional().is_empty() {
            return;
        }
        {
            let mut tail = self.tail.lock().unwrap_or_else(|p| p.into_inner());
            tail.finals.extend(update.finals.iter().cloned());
            tail.next_sequence = self
                .you
                .merger
                .next_sequence()
                .max(self.remote.merger.next_sequence());
            let mut provisional = self.remote.merger.provisional().to_vec();
            provisional.extend(self.you.merger.provisional().iter().cloned());
            tail.provisional = provisional;
        }
        for segment in update.finals.iter().chain(update.provisional.iter()) {
            self.publisher.segment(&self.meeting_id, segment);
        }
    }
}

enum LiveError {
    NoTimestamps(String),
    Engine(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_engine_proto::Backend;

    #[test]
    fn a_result_with_text_but_no_segments_names_the_engine() {
        let result = RecognizeResult {
            text: "hello".into(),
            language: "en".into(),
            segments: Vec::new(),
            duration_ms: 900,
            backend: Backend::Cpu,
        };
        let err = window_segments(result, "whisper").unwrap_err();
        assert!(err.contains("provider whisper"), "{err}");
        let empty = RecognizeResult {
            text: String::new(),
            language: "en".into(),
            segments: Vec::new(),
            duration_ms: 900,
            backend: Backend::Cpu,
        };
        assert!(window_segments(empty, "whisper").unwrap().is_empty());
    }
}
