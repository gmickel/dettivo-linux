//! The worker thread: drains both tracks into their takes on one clock,
//! feeds the live transcriber, meters them, journals every event, writes
//! the live checkpoint on the interval, restarts the microphone on the
//! new default when the device goes away, and ends the capture stopped,
//! cancelled or failed with the row, the sidecars and `metadata.json` in
//! step (`worker_end`), finalising after a stop.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::takes::TakeWriter;
use dettivo_audio::{EndReason, Event};
use dettivo_speech::SttEngine;
use dettivo_storage::meetings::MeetingRow;
use dettivo_storage::time::now_iso;

use crate::checkpoint::{Checkpoint, CheckpointTake};
use crate::journal::Journal;
use crate::live::{Command, LiveHandle};
use crate::machine::{Active, Finalizations, Shared};
use crate::source::{Source, Sources};
use crate::{Archive, Level, Policy, Publisher, State, Track};

/// How the loop ended.
pub(crate) enum Exit {
    Stopped,
    Cancelled,
    Failed(String),
}

/// One track's source and writer.
pub(crate) struct TrackState {
    pub(crate) track: Track,
    pub(crate) source: Option<Source>,
    pub(crate) writer: TakeWriter,
    pub(crate) retry_at: Option<Instant>,
}

pub(crate) struct Worker {
    pub(crate) job_id: String,
    pub(crate) row: MeetingRow,
    pub(crate) policy: Policy,
    pub(crate) sources: Arc<dyn Sources>,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) dir: PathBuf,
    pub(crate) clock: Instant,
    pub(crate) publisher: Arc<dyn Publisher>,
    pub(crate) archive: Arc<dyn Archive>,
    pub(crate) finalizing: Finalizations,
    pub(crate) live: Option<LiveHandle>,
}

impl Worker {
    /// Updates the shared snapshot (`None` leaves a field as it is) and,
    /// when the state changes, publishes.
    pub(crate) fn set(
        &self,
        shared: &Mutex<Option<Active>>,
        state: Option<State>,
        reason: Option<String>,
        takes: Option<u32>,
        system_audio: Option<bool>,
    ) {
        let counts = self.live.as_ref().map(|l| {
            let tail = l.tail();
            (tail.finals.len() as u64, tail.last_end_ms())
        });
        let mut g = shared.lock().unwrap_or_else(|p| p.into_inner());
        let Some(a) = g.as_mut() else {
            return;
        };
        let previous = a.snapshot.state;
        a.snapshot.duration_ms = self.clock.elapsed().as_millis() as u64;
        if let Some(t) = takes {
            a.snapshot.microphone_takes = t;
        }
        if let Some(s) = system_audio {
            a.snapshot.system_audio = s;
        }
        if let Some((count, last_end)) = counts {
            a.snapshot.live_segment_count = count;
            a.snapshot.live_last_end_ms = last_end;
        }
        if let Some(s) = state {
            a.snapshot.state = s;
            if let State::Failed = s {
                a.snapshot.error = reason.clone();
            }
        }
        let change = a
            .snapshot
            .change(state.unwrap_or(previous), previous, reason);
        drop(g);
        if state.is_some() {
            self.publisher.state(&change);
        }
    }

    /// Records both tracks. Each source arrives with the instant it opened,
    /// the origin its first samples have on the meeting clock: the take
    /// and the live windower both start there, so the live windows and
    /// the finalisation read one clock.
    pub(crate) fn run(
        mut self,
        mic: (Source, Instant),
        system: Option<(Source, Instant)>,
        shared: Shared,
    ) {
        let journal = Journal::open(&self.dir, self.clock);
        let mut mic = match self.open_track(Track::Microphone, Some(mic)) {
            Ok(t) => t,
            Err(e) => {
                self.finish(&shared, &journal, Exit::Failed(e), None, None);
                return;
            }
        };
        let mut sys = match self.open_track(Track::System, system) {
            Ok(t) => t,
            Err(e) => {
                self.finish(&shared, &journal, Exit::Failed(e), Some(mic), None);
                return;
            }
        };
        journal.record(
            "start",
            None,
            None,
            Some(format!(
                "system_audio={} keep_audio={} live={}",
                sys.source.is_some(),
                self.policy.keep_audio,
                self.policy.live
            )),
        );
        if sys.source.is_none() {
            journal.record(
                "system_unavailable",
                Some(Track::System),
                None,
                Some("no default sink monitor; room audio only".into()),
            );
        }
        let origin_ms = |t: &TrackState| {
            t.writer
                .takes()
                .takes
                .first()
                .map(|take| take.start_offset_ms)
                .unwrap_or(0)
        };
        self.start_live(&journal, (origin_ms(&mic), origin_ms(&sys)));
        self.row.microphone_takes = 1;
        self.set(&shared, None, None, Some(1), Some(sys.source.is_some()));
        let interval = self
            .policy
            .checkpoint_interval
            .max(Duration::from_millis(200));
        let mut next_checkpoint = Instant::now() + interval;
        let mut next_counts = Instant::now();
        let exit = loop {
            if self.cancel.load(Ordering::Relaxed) {
                break Exit::Cancelled;
            }
            if self.stop.load(Ordering::Relaxed) {
                break Exit::Stopped;
            }
            let mut busy = false;
            if mic.retry_at.is_some_and(|at| Instant::now() >= at)
                && let Err(e) =
                    self.switch_microphone(&mut mic, &journal, &shared, "microphone retry")
            {
                break Exit::Failed(e);
            }
            match self.drain(&mut mic, &journal, &shared) {
                Ok(b) => busy |= b,
                Err(e) => break Exit::Failed(e),
            }
            match self.drain(&mut sys, &journal, &shared) {
                Ok(b) => busy |= b,
                Err(e) => break Exit::Failed(e),
            }
            if Instant::now() >= next_checkpoint {
                self.checkpoint(&mut mic, &mut sys, &journal, &shared);
                next_checkpoint = Instant::now() + interval;
            } else if Instant::now() >= next_counts {
                self.set(&shared, None, None, None, None);
                next_counts = Instant::now() + Duration::from_millis(250);
            }
            if !busy {
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        self.finish(&shared, &journal, exit, Some(mic), Some(sys));
    }

    /// Spawns the live transcriber when the policy asks for it, its lanes
    /// starting at the first takes' origins (microphone, system); a
    /// thread that cannot start is journaled and the meeting records
    /// without it.
    fn start_live(&mut self, journal: &Journal, origins_ms: (u64, u64)) {
        if !self.policy.live {
            return;
        }
        match crate::live::spawn(
            &self.row.id,
            self.engine.clone(),
            self.policy.tuning.clone(),
            &self.policy.transcribe,
            &self.row.language,
            self.policy.prompt.clone(),
            self.publisher.clone(),
            journal.clone(),
            origins_ms,
            0,
        ) {
            Ok(handle) => self.live = Some(handle),
            Err(e) => journal.record(
                "live_unavailable",
                None,
                None,
                Some(format!("live thread: {e}")),
            ),
        }
    }

    fn open_track(
        &self,
        track: Track,
        source: Option<(Source, Instant)>,
    ) -> Result<TrackState, String> {
        let mut writer = TakeWriter::with_prefix(&self.dir, track.prefix(), self.clock)
            .map_err(|e| format!("meeting directory: {e}"))?;
        let source = match source {
            Some((source, opened)) => {
                writer
                    .start_take_at(opened, false)
                    .map_err(|e| format!("take: {e}"))?;
                Some(source)
            }
            None => None,
        };
        Ok(TrackState {
            track,
            source,
            writer,
            retry_at: None,
        })
    }

    /// Pulls every pending event of one track; true when any arrived. A
    /// take that cannot be written ends the meeting failed.
    fn drain(
        &self,
        t: &mut TrackState,
        journal: &Journal,
        shared: &Shared,
    ) -> Result<bool, String> {
        let mut busy = false;
        loop {
            let event = match t.source.as_ref() {
                Some(s) => s.events().try_recv(),
                None => return Ok(busy),
            };
            let event = match event {
                Ok(e) => e,
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(busy),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    t.source = None;
                    if t.track == Track::Microphone {
                        self.switch_microphone(t, journal, shared, "source disconnected")?;
                    }
                    return Ok(busy);
                }
            };
            busy = true;
            match event {
                Event::Pcm(samples) => {
                    if let Err(e) = t.writer.write(&samples) {
                        let reason = format!("{} take: {e}", t.track.prefix());
                        journal.record("error", Some(t.track), None, Some(reason.clone()));
                        return Err(reason);
                    }
                    if let Some(live) = &self.live {
                        live.send(Command::Pcm(t.track, samples));
                    }
                }
                Event::Level { rms, peak } => self.publisher.level(Level {
                    track: t.track,
                    rms,
                    peak,
                }),
                Event::DeviceChanged { from, to } => {
                    let detail = format!(
                        "{} -> {}",
                        from.as_deref().unwrap_or("none"),
                        to.as_deref().unwrap_or("none")
                    );
                    match t.track {
                        Track::Microphone => {
                            journal.record("device_switch", Some(t.track), None, Some(detail));
                            self.switch_microphone(t, journal, shared, "default source moved")?;
                        }
                        // The stream follows the default sink on its own;
                        // the switch is a fact for the journal, not a gap.
                        Track::System => {
                            journal.record("system_switch", Some(t.track), None, Some(detail));
                        }
                    }
                }
                Event::Ended { reason } => {
                    let why = match &reason {
                        EndReason::Stopped => "stopped".to_string(),
                        EndReason::DeviceLost(name) => format!("device lost: {name}"),
                        EndReason::NoSource => "no source left".to_string(),
                        EndReason::Error(e) | EndReason::Write(e) => format!("capture error: {e}"),
                        EndReason::FixtureFinished => "fixture finished".to_string(),
                    };
                    t.source = None;
                    match (t.track, &reason) {
                        (_, EndReason::Stopped) => {}
                        (Track::Microphone, EndReason::FixtureFinished) => {
                            journal.record("fixture_finished", Some(t.track), None, None);
                        }
                        (Track::Microphone, _) => {
                            journal.record("device_lost", Some(t.track), None, Some(why.clone()));
                            self.switch_microphone(t, journal, shared, &why)?;
                        }
                        (Track::System, _) => {
                            journal.record("system_ended", Some(t.track), None, Some(why));
                            self.set(shared, None, None, None, Some(false));
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn takes_of(&self, t: &TrackState) -> u32 {
        t.writer.takes().takes.len() as u32
    }

    /// Flushes both tracks and writes the checkpoint; a write that fails
    /// is logged and the next interval retries.
    fn checkpoint(
        &mut self,
        mic: &mut TrackState,
        sys: &mut TrackState,
        journal: &Journal,
        shared: &Shared,
    ) {
        for t in [&mut *mic, &mut *sys] {
            if let Err(e) = t.writer.flush() {
                tracing::warn!(error = %e, track = t.track.prefix(), "meeting: take not flushed");
            }
        }
        let checkpoint = self.checkpoint_of(mic, sys, None);
        match checkpoint.write(&self.dir) {
            Ok(()) => journal.record(
                "checkpoint",
                None,
                None,
                Some(format!(
                    "takes={} segments={}",
                    checkpoint.takes.len(),
                    checkpoint.segments.len()
                )),
            ),
            Err(e) => {
                tracing::warn!(error = %e, "meeting: checkpoint not written; retrying next interval")
            }
        }
        self.row.duration_ms = self.clock.elapsed().as_millis() as u64;
        self.row.microphone_takes = self.takes_of(mic);
        if let Err(e) = self.archive.updated(&self.row) {
            tracing::warn!(error = %e, "meeting: row not updated at the checkpoint");
        }
        self.set(
            shared,
            None,
            None,
            Some(self.row.microphone_takes),
            Some(sys.source.is_some()),
        );
    }

    pub(crate) fn checkpoint_of(
        &self,
        mic: &TrackState,
        sys: &TrackState,
        error: Option<String>,
    ) -> Checkpoint {
        let mut c = Checkpoint::new(&self.row.id, &self.row.started_at);
        c.updated_at = now_iso();
        for (track, t) in [(Track::Microphone, mic), (Track::System, sys)] {
            c.takes.extend(
                t.writer
                    .takes()
                    .takes
                    .iter()
                    .map(|take| CheckpointTake::from_take(track, take)),
            );
        }
        if let Some(live) = &self.live {
            let tail = live.tail();
            let mut finals = tail.finals.clone();
            finals.sort_by_key(|s| (s.start_ms, s.end_ms));
            c.segments = dettivo_transcribe::live::contract_segments(&finals);
            c.next_sequence = tail.next_sequence;
        }
        c.last_error = error;
        c
    }
}
