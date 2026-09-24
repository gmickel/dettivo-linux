//! Where a session's audio comes from: a PipeWire capture, the mock
//! microphone fixture (`DETTIVO_MOCK_MIC`), or a channel the tests feed.

use std::sync::mpsc::Receiver;

use dettivo_audio::Event;
use dettivo_audio::mock::MockCapture;
use dettivo_audio::service::CaptureHandle;

/// A running audio source the session drains.
pub enum Source {
    /// A PipeWire capture.
    Capture(CaptureHandle),
    /// The fixture player.
    Mock(MockCapture),
    /// A channel (tests).
    Channel {
        /// The events.
        events: Receiver<Event>,
        /// Requests stop; must emit `Ended` after its final accepted PCM.
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

    /// Requests stop. Drain events through `Ended` to acknowledge the boundary.
    pub fn stop(&self) {
        match self {
            Self::Capture(c) => c.stop(),
            Self::Mock(m) => m.stop(),
            Self::Channel { on_stop, .. } => on_stop(),
        }
    }
}

/// Opens the source for a session.
pub trait SourceFactory: Send + Sync {
    /// Opens a source, or says why not (the device, PipeWire, the fixture).
    fn open(&self, level_interval_ms: u64) -> Result<Source, String>;
}
