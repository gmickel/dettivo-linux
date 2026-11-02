//! The recording phase: drains the source into the take until a cancel,
//! the maximum duration, the source's end, or the release tail after a
//! stop (ADR 0092). The tail keeps recording until audio received at or
//! after `released + release_tail` arrived, or audio stopped arriving for
//! [`TAIL_STALL_GRACE`]; then the source stops and the take ends with the
//! final chunk it delivered (ADR 0082's one-quantum tolerance).

use std::sync::atomic::Ordering;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use dettivo_audio::takes::TakeWriter;
use dettivo_audio::{EndReason, Event};

use super::{Worker, elapsed_ms};
use crate::machine::{Exit, SharedState};
use crate::source::Source;
use crate::{Level, TAIL_STALL_GRACE};

/// What the recording phase hands to the rest of the worker.
pub(super) struct Recorded {
    pub(super) samples: Vec<i16>,
    pub(super) duration_ms: u64,
    /// An early exit when the recording itself decided the outcome.
    pub(super) exit: Option<Exit>,
    /// From the stop being observed to the take being final.
    pub(super) capture_ms: u64,
}

/// The release tail once a stop was seen.
struct Tail {
    /// Audio received at or after this covers the tail.
    until: Instant,
    /// When the worker saw the stop.
    seen: Instant,
}

impl Worker {
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
        let started = Instant::now();
        let mut heard = started;
        let mut tail: Option<Tail> = None;
        let mut ended: Option<EndReason> = None;
        let mut terminal = false;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Recorded {
                    samples,
                    duration_ms: 0,
                    exit: Some(self.cancelled()),
                    capture_ms: 0,
                });
            }
            if tail.is_none() && self.stop.load(Ordering::Relaxed) {
                let seen = Instant::now();
                let released = self.released.get().copied().unwrap_or(seen);
                tail = Some(Tail {
                    until: released + self.policy.release_tail,
                    seen,
                });
            }
            let wait = match &tail {
                Some(t) => {
                    let quiet = Instant::now().saturating_duration_since(heard.max(t.seen));
                    if heard >= t.until || quiet >= TAIL_STALL_GRACE {
                        break;
                    }
                    Duration::from_millis(10)
                }
                None if started.elapsed() >= self.policy.max_duration => break,
                None => Duration::from_millis(50),
            };
            match src.events().recv_timeout(wait) {
                Ok(Event::Pcm(chunk)) => {
                    heard = Instant::now();
                    writer
                        .write(&chunk)
                        .map_err(|e| Exit::Failed(format!("take: {e}")))?;
                    samples.extend_from_slice(&chunk);
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
        let released = tail.map_or_else(Instant::now, |t| t.seen);
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
                Event::Pcm(chunk) => {
                    writer
                        .write(&chunk)
                        .map_err(|e| Exit::Failed(format!("take: {e}")))?;
                    samples.extend_from_slice(&chunk);
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
        })
    }
}
