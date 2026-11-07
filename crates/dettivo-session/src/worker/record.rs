//! The recording phase: drains the source until a cancel, the maximum
//! duration, the source's end, or the release tail after a stop (ADR
//! 0093). The tail keeps recording until the source delivered audio
//! captured at or after `released + release_tail`, or audio stopped
//! arriving for [`TAIL_STALL_GRACE`]. The take is then cut at that time by
//! the chunks' capture stamps, so audio a release that queued behind a
//! slow start would otherwise let in never reaches the take file or the
//! engine. A take whose last stretch before the release is silent ends at
//! the release instead (`ending`).

use std::sync::atomic::Ordering;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use dettivo_audio::takes::TakeWriter;
use dettivo_audio::{EndReason, Event};

use super::check::SpeechCheck;
use super::ending::{self, Early};
use super::held::{Held, HeldRounds};
use super::release::Ahead;
use super::{Worker, elapsed_ms};
use crate::finish::{Exit, SharedState};
use crate::source::Source;
use crate::{Level, TAIL_STALL_GRACE};
use dettivo_speech::gate::VAD_WINDOW;

const SAMPLES_PER_SECOND: u128 = 16_000;

/// What the recording phase hands to the rest of the worker.
pub(super) struct Recorded {
    pub(super) samples: Vec<i16>,
    pub(super) duration_ms: u64,
    /// An early exit when the recording itself decided the outcome.
    pub(super) exit: Option<Exit>,
    /// From the stop being observed to the take being final.
    pub(super) capture_ms: u64,
    /// When the stop was observed.
    pub(super) stopped: Instant,
    /// The speech check that ran during the take, when the detector streams.
    pub(super) check: Option<SpeechCheck>,
    /// The rounds that decoded the take while it was held (ADR 0102),
    /// and their tail decode, started once the take's samples were final.
    pub(super) held: Option<Held>,
    pub(super) ahead: Option<Ahead>,
}

/// The release tail once a stop was seen.
struct Tail {
    /// When the key was released.
    released: Instant,
    /// Audio received at or after this covers the tail.
    until: Instant,
    /// When the worker saw the stop.
    seen: Instant,
    /// The early end on a silent ending.
    early: Early,
}

impl Worker {
    /// True while any release that would make this take a tap (it came
    /// after the press and before `qualifies`) waits behind the start.
    fn tap_waiting(&self, qualifies: Instant) -> bool {
        self.release_waiting
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .any(|&at| at >= self.pressed && at < qualifies)
    }

    /// Drains the source until stop, cancel, the maximum duration or the
    /// source's end. Returns the samples, the duration, an early exit when
    /// the recording itself decided the outcome, and how long the tail,
    /// the drain and the take's finish took after the stop.
    pub(super) fn record(&self, src: &Source, shared: &SharedState) -> Result<Recorded, Exit> {
        let mut writer =
            TakeWriter::new(&self.dir).map_err(|e| Exit::Failed(format!("take: {e}")))?;
        writer
            .start_take(false)
            .map_err(|e| Exit::Failed(format!("take: {e}")))?;
        let mut samples: Vec<i16> = Vec::new();
        // Each chunk's end in `samples` and when it was captured.
        let mut stamps: Vec<(usize, Instant)> = Vec::new();
        let started = Instant::now();
        // When the newest audio was captured, and when it arrived here.
        let (mut heard, mut arrived) = (None::<Instant>, started);
        let mut tail: Option<Tail> = None;
        // Where a silent ending cut the take, at the release.
        let mut early_cut: Option<usize> = None;
        // A tail shorter than one voice window has nothing to end early.
        let ends_early = self.policy.release_tail.as_micros() * SAMPLES_PER_SECOND / 1_000_000
            > VAD_WINDOW as u128;
        let mut ended: Option<EndReason> = None;
        let mut terminal = false;
        // The check starts once the press qualifies as a take, with what
        // was captured until then: a tap never reaches the detector (ADR
        // 0093). `fed` counts the samples sent to it.
        let qualifies = self.pressed + self.policy.min_hold;
        let mut detector = self.vad.as_deref();
        let mut check: Option<SpeechCheck> = None;
        let mut fed = 0;
        // The rounds start with the check, for the same reason (ADR 0102).
        let mut rounds: Option<HeldRounds> = None;
        let (mut rounds_fed, mut rounds_decided) = (0, false);
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Recorded {
                    samples,
                    duration_ms: 0,
                    exit: Some(self.cancelled()),
                    capture_ms: 0,
                    stopped: Instant::now(),
                    check: None,
                    held: None,
                    ahead: None,
                });
            }
            if tail.is_none() && self.stop.load(Ordering::Relaxed) {
                let seen = Instant::now();
                let released = self.released.get().copied().unwrap_or(seen);
                if let Some(r) = &rounds {
                    r.release();
                }
                tail = Some(Tail {
                    released,
                    until: released + self.policy.release_tail,
                    seen,
                    early: if ends_early {
                        Early::Pending
                    } else {
                        Early::Kept
                    },
                });
            }
            let wait = match &mut tail {
                Some(t) => {
                    let quiet = Instant::now().saturating_duration_since(arrived.max(t.seen));
                    let tail_done =
                        heard.is_some_and(|h| h >= t.until) || quiet >= TAIL_STALL_GRACE;
                    let heard_now = ending::Heard {
                        samples: &samples,
                        at_release: ending::past(heard, t.released)
                            .then(|| cut_at(&stamps, t.released)),
                        silence_dbfs: self.policy.silence_dbfs,
                        check: check.as_ref(),
                        tail_done,
                    };
                    if let Some(end) = t.early.advance(heard_now, &mut fed) {
                        early_cut = Some(end);
                        break;
                    }
                    if tail_done {
                        break;
                    }
                    // The check answers within a few milliseconds.
                    Duration::from_millis(if t.early.asked() { 1 } else { 10 })
                }
                None if started.elapsed() >= self.policy.max_duration => break,
                None => Duration::from_millis(50),
            };
            match src.events().recv_timeout(wait) {
                Ok(Event::Pcm { pcm: chunk, at }) => {
                    (heard, arrived) = (Some(at), Instant::now());
                    samples.extend_from_slice(&chunk);
                    stamps.push((samples.len(), at));
                    // Audio that arrives after the stop is seen may be cut
                    // from the take, so the check runs it at the release.
                    if tail.is_none() {
                        let qualified = arrived >= qualifies
                            && !self.cancel.load(Ordering::Relaxed)
                            && !self.tap_waiting(qualifies);
                        if check.is_none()
                            && qualified
                            && let Some(vad) = detector.take()
                        {
                            check = vad.stream().map(SpeechCheck::start);
                        }
                        if let Some(c) = &check {
                            c.feed(&samples[fed..]);
                            fed = samples.len();
                        }
                        if !rounds_decided && qualified {
                            rounds_decided = true;
                            rounds = self
                                .rounds_wanted()
                                .then(|| HeldRounds::start(self.engine.clone(), self.request()));
                        }
                        if let Some(r) = &rounds {
                            r.feed(&samples[rounds_fed..]);
                            rounds_fed = samples.len();
                        }
                    }
                    let progress = (started.elapsed().as_secs_f64()
                        / self.policy.max_duration.as_secs_f64())
                    .min(1.0);
                    if let Some(a) = shared
                        .active
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                    {
                        a.snapshot.progress = progress;
                    }
                }
                Ok(Event::Level { rms, peak: p }) => {
                    self.publisher.level(Level { rms, peak: p });
                }
                Ok(Event::DeviceChanged { from, to }) => {
                    ended = Some(EndReason::DeviceLost(format!(
                        "default moved from {} to {}",
                        from.unwrap_or_else(|| "none".into()),
                        to.unwrap_or_else(|| "none".into())
                    )));
                    break;
                }
                Ok(Event::Ended { reason }) => {
                    ended = Some(reason);
                    terminal = true;
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    ended = Some(EndReason::Error(
                        "capture stop acknowledgement missing".into(),
                    ));
                    terminal = true;
                    break;
                }
            }
        }
        let released = tail.as_ref().map_or_else(Instant::now, |t| t.seen);
        // Where the take ends is known once the tail is covered: the rounds
        // end and their tail decode starts while the capture stops and the
        // take is written. The release checks the take kept that length.
        let held = rounds.and_then(HeldRounds::finish);
        let ahead = tail
            .as_ref()
            .filter(|_| ended.is_none())
            .map(|t| {
                early_cut.map_or_else(
                    || cut_at(&stamps, t.until),
                    |end| end.min(cut_at(&stamps, t.until)),
                )
            })
            .and_then(|end| self.decode_ahead(&samples[..end.min(samples.len())], held.as_ref()));
        src.stop();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !terminal {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Exit::Failed(
                    "capture stop acknowledgement timed out".into(),
                ));
            }
            let event = src.events().recv_timeout(remaining).map_err(|_| {
                Exit::Failed("capture stop acknowledgement missing or timed out".into())
            })?;
            match event {
                Event::Pcm { pcm: chunk, at } => {
                    samples.extend_from_slice(&chunk);
                    stamps.push((samples.len(), at));
                }
                Event::Level { rms, peak: p } => {
                    self.publisher.level(Level { rms, peak: p });
                }
                Event::Ended { reason } => {
                    terminal = true;
                    if ended.is_none() {
                        ended = Some(reason);
                    }
                }
                _ => {}
            }
        }
        if let Some(t) = &tail {
            // An early end never keeps audio the tail would have cut.
            let tail_cut = cut_at(&stamps, t.until);
            samples.truncate(early_cut.map_or(tail_cut, |end| end.min(tail_cut)));
        }
        writer
            .write(&samples)
            .map_err(|e| Exit::Failed(format!("take: {e}")))?;
        let takes = writer
            .finish()
            .map_err(|e| Exit::Failed(format!("take: {e}")))?;
        let duration_ms = takes.takes.iter().map(|t| t.samples).sum::<u64>() * 1000 / 16_000;
        let exit = match ended {
            Some(EndReason::DeviceLost(name)) => Some(Exit::Failed(format!("device lost: {name}"))),
            Some(EndReason::NoSource) => Some(Exit::Failed("no audio source".into())),
            Some(EndReason::Error(e)) | Some(EndReason::Write(e)) => {
                Some(Exit::Failed(format!("capture error: {e}")))
            }
            Some(EndReason::Stopped) | Some(EndReason::FixtureFinished) | None => None,
        };
        Ok(Recorded {
            samples,
            duration_ms,
            exit,
            capture_ms: elapsed_ms(released),
            stopped: released,
            check,
            held,
            ahead,
        })
    }
}

/// How many samples were captured at or before `until`: a chunk whose
/// last sample came after it keeps the samples up to it, and every later
/// chunk goes.
fn cut_at(stamps: &[(usize, Instant)], until: Instant) -> usize {
    let mut start = 0;
    for &(end, at) in stamps {
        if at > until {
            let late =
                at.saturating_duration_since(until).as_micros() * SAMPLES_PER_SECOND / 1_000_000;
            return end.saturating_sub(late as usize).max(start);
        }
        start = end;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_take_is_cut_at_the_capture_time_of_the_tail_end() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        // Three 20 ms chunks captured by 20, 40 and 60 ms.
        let stamps = [(320, ms(20)), (640, ms(40)), (960, ms(60))];
        assert_eq!(cut_at(&stamps, ms(60)), 960);
        assert_eq!(cut_at(&stamps, ms(30)), 480, "mid-chunk, to the sample");
        assert_eq!(cut_at(&stamps, ms(5)), 80);
        assert_eq!(cut_at(&stamps, t0), 0, "nothing before the tail end");
        // A one-second backlog delivered at once, captured by 1000 ms.
        assert_eq!(cut_at(&[(16_000, ms(1_000))], ms(700)), 11_200);
    }
}
