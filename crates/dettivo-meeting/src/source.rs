//! Where a meeting's audio comes from: a PipeWire capture per track, the
//! mock fixtures (`DETTIVO_MOCK_MIC`, `DETTIVO_MOCK_SYSTEM_AUDIO`), or a
//! channel the tests feed.

use std::sync::mpsc::Receiver;

use dettivo_audio::Event;
use dettivo_audio::mock::MockCapture;
use dettivo_audio::service::CaptureHandle;

/// A running audio source the worker drains.
pub enum Source {
    /// A PipeWire capture.
    Capture(CaptureHandle),
    /// A fixture player.
    Mock(MockCapture),
    /// A channel (tests).
    Channel {
        /// The events.
        events: Receiver<Event>,
        /// Called on stop.
        on_stop: Box<dyn Fn() + Send>,
    },
}

impl Source {
    /// The event stream.
    pub fn events(&self) -> &Receiver<Event> {
        match self {
            Self::Capture(c) => c.events(),
            Self::Mock(m) => m.events(),
            Self::Channel { events, .. } => events,
        }
    }

    /// Stops the source; the stream ends.
    pub fn stop(&self) {
        match self {
            Self::Capture(c) => c.stop(),
            Self::Mock(m) => m.stop(),
            Self::Channel { on_stop, .. } => on_stop(),
        }
    }
}

/// Opens the two tracks. The microphone is required at start; the
/// system track is optional and a meeting without it is room audio.
pub trait Sources: Send + Sync {
    /// Opens the microphone: the pinned device, else the default. After a
    /// device loss (`reopen`) a pinned device that is gone falls back to
    /// the default; an error names why nothing could open.
    fn open_microphone(&self, level_interval_ms: u64, reopen: bool) -> Result<Source, String>;
    /// Opens the system track (the default sink's monitor, or the pinned
    /// sink's); an error names why it is unavailable.
    fn open_system(&self, level_interval_ms: u64) -> Result<Source, String>;
}
