//! Wire protocol between the daemon and engine processes (ADR 0003): one
//! framed message stream over the child's stdio, length-prefixed JSON
//! headers with binary attachments (16 kHz mono PCM), versioned, and
//! fixture-tested the way the IPC contract is. Every engine binary speaks
//! it on stdin/stdout and logs on stderr; the daemon's supervisor speaks
//! it from the other side. `host` is the engine side of that loop and
//! `host_llm` its streaming counterpart for the language model binary,
//! `host_diarize` the one for the diarization binary (a `cancel` and a
//! `status` reach it while a pass runs), and `backend` the shared
//! Vulkan-or-CPU choice, so every engine binary answers the same way.

pub mod backend;
pub mod frame;
pub mod host;
pub mod host_diarize;
pub mod host_llm;
pub mod messages;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The protocol version every frame carries.
pub const VERSION: u32 = 1;

pub use frame::{
    Attachment, Frame, FrameError, Kind, MAX_PCM_SAMPLES, MAX_PCM_SECONDS, bytes_to_pcm,
    pcm_to_bytes, read_frame, validate_pcm_samples, write_frame,
};
pub use host::{EngineError, SpeechEngine};
pub use host_diarize::DiarizeEngine;
pub use host_llm::LanguageEngine;
pub use messages::*;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-engine-proto");
    }
}
