//! Microphone replacement and paced reopening while the device is absent.

use std::time::{Duration, Instant};

use crate::journal::Journal;
use crate::live::Command;
use crate::machine::Shared;
use crate::worker::{TrackState, Worker};
use crate::{State, Track};

impl Worker {
    pub(crate) fn switch_microphone(
        &self,
        t: &mut TrackState,
        journal: &Journal,
        shared: &Shared,
        why: &str,
    ) -> Result<(), String> {
        if let Some(source) = t.source.take() {
            source.stop();
        }
        t.writer
            .finish_take()
            .map_err(|e| format!("microphone take not closed: {e}"))?;
        let previous_end_ms = t
            .writer
            .takes()
            .takes
            .last()
            .map(|take| {
                take.start_offset_ms + take.samples * 1000 / u64::from(dettivo_audio::SAMPLE_RATE)
            })
            .unwrap_or(0);
        match self
            .sources
            .open_microphone(self.policy.level_interval_ms, true)
        {
            Ok(source) => {
                t.source = Some(source);
                let take = t
                    .writer
                    .start_take(true)
                    .map_err(|e| format!("microphone take: {e}"))?;
                t.retry_at = None;
                let file = take.file.clone();
                let origin_ms = take.start_offset_ms;
                journal.record(
                    "gap",
                    Some(Track::Microphone),
                    Some(&file),
                    Some(format!("{why}; new take on the current default")),
                );
                if let Some(live) = &self.live {
                    live.send(Command::Restart {
                        track: Track::Microphone,
                        origin_ms,
                        gap_ms: origin_ms.saturating_sub(previous_end_ms),
                    });
                }
                self.set(
                    shared,
                    Some(State::Recording),
                    Some(format!("microphone restarted: {file}")),
                    Some(self.takes_of(t)),
                    None,
                );
            }
            Err(e) => {
                if t.retry_at.is_none() {
                    journal.record(
                        "gap",
                        Some(Track::Microphone),
                        None,
                        Some(format!("{why}; no device: {e}")),
                    );
                }
                t.retry_at = Some(Instant::now() + Duration::from_millis(500));
            }
        }
        Ok(())
    }
}
