//! Cutting one source into windows on the meeting clock. Every `tick_ms`
//! of new audio cuts a window that reaches back `overlap_ms` into the
//! previous one and forward to the newest sample, never longer than
//! `window_ms`; an engine that fell more than two ticks behind has the
//! oldest audio skipped so the next window is current. A window under the
//! speech floor (the continuation floor while the previous window was
//! speech) is silence and never reaches the engine. Time is counted in
//! samples from the take's offset, so the windows line up with the take
//! files finalisation reads.

use crate::SAMPLE_RATE;
use crate::filters::rms;
use crate::live::{LiveSettings, Source};

/// The shortest tail flushed at stop, in milliseconds (the macOS
/// `minFinalFlushWindowMs`).
pub const MIN_FLUSH_MS: u64 = 180;

/// One window ready for the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// Which side.
    pub source: Source,
    /// Start on the meeting clock.
    pub start_ms: u64,
    /// End on the meeting clock.
    pub end_ms: u64,
    /// The 16 kHz samples.
    pub samples: Vec<i16>,
}

/// What a tick produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Cut {
    /// A window for the engine.
    Window(Window),
    /// A window under the speech floor; no engine call.
    Silent {
        /// Start on the meeting clock.
        start_ms: u64,
        /// End on the meeting clock.
        end_ms: u64,
    },
    /// Audio the engine never sees because it fell too far behind.
    Skipped {
        /// Start on the meeting clock.
        start_ms: u64,
        /// End on the meeting clock.
        end_ms: u64,
    },
}

fn samples_of(ms: u64) -> u64 {
    ms * SAMPLE_RATE / 1000
}

fn ms_of(samples: u64) -> u64 {
    samples * 1000 / SAMPLE_RATE
}

/// The windower of one source.
pub struct Windower {
    source: Source,
    settings: LiveSettings,
    buffer: Vec<i16>,
    /// The meeting-clock sample index of `buffer[0]`.
    buffer_start: u64,
    /// The meeting-clock sample index the last window ended at.
    cursor: u64,
    /// The sample index the current take began at.
    origin: u64,
    speaking: bool,
}

impl Windower {
    /// A windower whose first take began `origin_ms` into the meeting.
    pub fn new(source: Source, settings: LiveSettings, origin_ms: u64) -> Self {
        let origin = samples_of(origin_ms);
        Self {
            source,
            settings,
            buffer: Vec::new(),
            buffer_start: origin,
            cursor: origin,
            origin,
            speaking: false,
        }
    }

    /// Which side.
    pub fn source(&self) -> Source {
        self.source
    }

    /// The meeting-clock end of the audio received so far, in milliseconds.
    pub fn end_ms(&self) -> u64 {
        ms_of(self.buffer_start + self.buffer.len() as u64)
    }

    /// Appends samples of the current take.
    pub fn push(&mut self, samples: &[i16]) {
        self.buffer.extend_from_slice(samples);
    }

    /// Milliseconds of audio no window has covered yet.
    pub fn pending_ms(&self) -> u64 {
        ms_of((self.buffer_start + self.buffer.len() as u64).saturating_sub(self.cursor))
    }

    /// A new take began `origin_ms` into the meeting (a device switch):
    /// the previous take's tail is flushed and returned, then the clock
    /// jumps to the new offset.
    pub fn restart(&mut self, origin_ms: u64) -> Vec<Cut> {
        let cuts = self.flush();
        let origin = samples_of(origin_ms);
        self.buffer.clear();
        self.buffer_start = origin;
        self.cursor = origin;
        self.origin = origin;
        self.speaking = false;
        cuts
    }

    fn available_end(&self) -> u64 {
        self.buffer_start + self.buffer.len() as u64
    }

    fn cut_to(&mut self, end: u64) -> Cut {
        let overlap = samples_of(self.settings.overlap_ms);
        let start = self.cursor.saturating_sub(overlap).max(self.origin);
        let from = (start - self.buffer_start) as usize;
        let to = (end - self.buffer_start) as usize;
        let samples = self.buffer[from..to].to_vec();
        self.cursor = end;
        let floor = if self.speaking {
            self.settings.continuation_floor_rms()
        } else {
            self.settings.speech_floor_rms
        };
        let cut = if rms(&samples) < floor {
            self.speaking = false;
            Cut::Silent {
                start_ms: ms_of(start),
                end_ms: ms_of(end),
            }
        } else {
            self.speaking = true;
            Cut::Window(Window {
                source: self.source,
                start_ms: ms_of(start),
                end_ms: ms_of(end),
                samples,
            })
        };
        // Keep the overlap for the next window and drop the rest.
        let keep_from = self.cursor.saturating_sub(overlap).max(self.buffer_start);
        let drop = (keep_from - self.buffer_start) as usize;
        self.buffer.drain(..drop);
        self.buffer_start = keep_from;
        cut
    }

    /// Cuts every window that is due: one per `tick_ms` of new audio,
    /// each reaching back `overlap_ms` and forward to the newest sample,
    /// at most `window_ms` long. More than two ticks of lag beyond a
    /// window skips the oldest audio first.
    pub fn cut(&mut self) -> Vec<Cut> {
        let tick = samples_of(self.settings.tick_ms);
        let window = samples_of(self.settings.window_ms);
        let overlap = samples_of(self.settings.overlap_ms);
        let stride = window - overlap;
        let mut cuts = Vec::new();
        loop {
            let end = self.available_end();
            let pending = end.saturating_sub(self.cursor);
            if pending < tick {
                return cuts;
            }
            if pending > window + 2 * tick {
                let skip_to = end - stride;
                cuts.push(Cut::Skipped {
                    start_ms: ms_of(self.cursor),
                    end_ms: ms_of(skip_to),
                });
                self.cursor = skip_to;
                self.speaking = false;
            }
            let new_end = self.available_end().min(self.cursor + stride);
            cuts.push(self.cut_to(new_end));
        }
    }

    /// Cuts whatever is pending at stop when it is at least
    /// [`MIN_FLUSH_MS`] long; shorter tails are dropped.
    pub fn flush(&mut self) -> Vec<Cut> {
        let mut cuts = self.cut();
        let end = self.available_end();
        if end.saturating_sub(self.cursor) >= samples_of(MIN_FLUSH_MS) {
            cuts.push(self.cut_to(end));
        }
        self.cursor = end;
        cuts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: u64, amplitude: i16) -> Vec<i16> {
        (0..samples_of(ms))
            .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
            .collect()
    }

    fn spans(cuts: &[Cut]) -> Vec<(&'static str, u64, u64)> {
        cuts.iter()
            .map(|c| match c {
                Cut::Window(w) => ("window", w.start_ms, w.end_ms),
                Cut::Silent { start_ms, end_ms } => ("silent", *start_ms, *end_ms),
                Cut::Skipped { start_ms, end_ms } => ("skipped", *start_ms, *end_ms),
            })
            .collect()
    }

    #[test]
    fn windows_cut_per_tick_with_the_overlap_and_never_exceed_the_target() {
        let mut w = Windower::new(Source::You, LiveSettings::default(), 0);
        w.push(&tone(800, 3000));
        assert!(w.cut().is_empty(), "under one tick nothing is cut");
        w.push(&tone(100, 3000));
        let first = w.cut();
        assert_eq!(spans(&first), [("window", 0, 900)]);
        w.push(&tone(900, 3000));
        let second = w.cut();
        assert_eq!(
            spans(&second),
            [("window", 450, 1800)],
            "reaches back the overlap"
        );
        if let Cut::Window(win) = &second[0] {
            assert_eq!(win.samples.len() as u64, samples_of(1350));
        }
        // Two ticks arrive at once (the engine held the loop): one window
        // absorbs both, still under the target.
        w.push(&tone(1800, 3000));
        assert_eq!(spans(&w.cut()), [("window", 1350, 3600)]);
        assert_eq!(w.pending_ms(), 0);
        assert_eq!(w.end_ms(), 3600);
    }

    #[test]
    fn a_lagging_engine_skips_the_oldest_audio_and_silence_is_gated() {
        let mut w = Windower::new(Source::Remote, LiveSettings::default(), 0);
        w.push(&tone(6000, 3000));
        let cuts = w.cut();
        assert_eq!(
            spans(&cuts),
            [("skipped", 0, 3450), ("window", 3000, 6000)],
            "more than two ticks beyond a window skips, then one full window"
        );
        // Silence under the floor never reaches the engine; the
        // continuation floor applies right after speech.
        let mut w = Windower::new(Source::You, LiveSettings::default(), 0);
        w.push(&tone(900, 0));
        assert_eq!(spans(&w.cut()), [("silent", 0, 900)]);
        w.push(&tone(900, 3000));
        assert_eq!(spans(&w.cut()), [("window", 450, 1800)]);
        w.push(&tone(900, 170)); // rms 0.0052: over the continuation floor, under activation
        assert_eq!(spans(&w.cut()), [("window", 1350, 2700)]);
        w.push(&tone(900, 100)); // rms 0.003: under both
        assert_eq!(spans(&w.cut()), [("silent", 2250, 3600)]);
        w.push(&tone(900, 170));
        assert_eq!(
            spans(&w.cut()),
            [("silent", 3150, 4500)],
            "activation floor again"
        );
    }

    #[test]
    fn a_restart_flushes_the_old_take_and_counts_from_the_new_offset() {
        let mut w = Windower::new(Source::You, LiveSettings::default(), 0);
        w.push(&tone(1200, 3000));
        let flushed = w.restart(5000);
        assert_eq!(
            spans(&flushed),
            [("window", 0, 1200)],
            "the pending tick and the tail in one window"
        );
        w.push(&tone(900, 3000));
        assert_eq!(spans(&w.cut()), [("window", 5000, 5900)]);
        w.push(&tone(100, 3000));
        assert!(w.flush().is_empty(), "a tail under 180 ms is dropped");
        w.push(&tone(200, 3000));
        assert_eq!(spans(&w.flush()), [("window", 5550, 6200)]);
        assert_eq!(w.pending_ms(), 0);
    }
}
