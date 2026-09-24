//! Audio capture for the daemon (ADR 0006): PipeWire directly, the default
//! source followed unless a device is pinned, 16 kHz mono requested from
//! PipeWire so no resampler runs here, levels for the meters, takes as
//! WAV files, and a fixture-backed capture for QA mode that speaks the
//! same API. `decode` turns every import content type into the same
//! 16 kHz mono stream through Symphonia and `resample` (ADR 0022).

pub mod capture;
pub mod decode;
pub mod graph;
pub mod level;
pub mod mock;
pub mod playback;
pub mod resample;
pub mod service;
pub mod takes;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME];

/// The sample rate every capture delivers.
pub const SAMPLE_RATE: u32 = 16_000;

/// What a capture wants to listen to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The default source, followed when it changes.
    Default,
    /// A pinned PipeWire node by `node.name`; a sink is captured through
    /// its monitor.
    Node(String),
    /// The monitor of the default sink (a meeting's system track),
    /// followed when the default sink changes.
    SystemMonitor,
}

/// One thing a running capture reports.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A chunk of 16 kHz mono signed 16-bit samples, always before `Ended`.
    Pcm(Vec<i16>),
    /// One meter sample over the configured window.
    Level {
        /// Root mean square, 0..1.
        rms: f32,
        /// Peak absolute sample, 0..1.
        peak: f32,
    },
    /// The default source changed while this capture followed it.
    DeviceChanged {
        /// The node captured before, when known.
        from: Option<String>,
        /// The node the default resolves to now, when any.
        to: Option<String>,
    },
    /// Capture ended.
    Ended {
        /// Why, for the session layer and the log.
        reason: EndReason,
    },
}

/// Why a capture ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    /// `stop` was called.
    Stopped,
    /// The pinned device disappeared; names it.
    DeviceLost(String),
    /// No source is left to follow.
    NoSource,
    /// PipeWire reported an error.
    Error(String),
    /// The fixture file was played to its end.
    FixtureFinished,
    /// The take file could not be written (a full disk); names the cause.
    Write(String),
}

/// Why a capture could not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// PipeWire is not reachable; the message names it.
    NoPipeWire(String),
    /// The pinned device is not present; names it.
    UnknownDevice(String),
    /// A fixture file could not be read.
    Fixture(String),
    /// PipeWire refused the stream.
    Stream(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPipeWire(m) => write!(f, "PipeWire is not reachable: {m}"),
            Self::UnknownDevice(d) => write!(f, "audio device {d:?} is not present"),
            Self::Fixture(m) => write!(f, "fixture: {m}"),
            Self::Stream(m) => write!(f, "PipeWire stream: {m}"),
        }
    }
}

impl std::error::Error for CaptureError {}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-audio");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto"]);
    }
}
