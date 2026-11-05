//! How much silence the warm-up after a GPU load sends each speech engine
//! (ADR 0084, ADR 0098). The supervisor sends it and the benchmark sends
//! the same, so the bench measures the warm-up the daemon runs.

/// The warm-up's silence: 30 s at 16 kHz, the longest window Whisper
/// decodes at once (ADR 0084).
pub const SAMPLES: usize = 480_000;

/// Parakeet's warm-up: 90 s, the longest input it runs in one pass. Its
/// Vulkan kernels differ by input length, and a shorter warm-up leaves
/// the first take past 30 s to compile its own, 2.5-2.9 s on the reference
/// GPU after a driver or engine update emptied the shader cache (ADR 0098).
pub const PARAKEET_SAMPLES: usize = 1_440_000;

/// The engine binary whose warm-up is `PARAKEET_SAMPLES`.
pub const PARAKEET_BINARY: &str = "dettivo-engine-parakeet";

/// The warm-up length for the engine binary named `binary`.
pub fn samples(binary: &str) -> usize {
    if binary == PARAKEET_BINARY {
        PARAKEET_SAMPLES
    } else {
        SAMPLES
    }
}
