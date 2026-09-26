//! Which model set and backend a speaker pass runs (ADR 0073): Nemotron
//! when it is configured, downloaded and the expected count fits its eight
//! channels, the sherpa-onnx set with the reason otherwise.

use dettivo_core::config::schema::DiarizeBackend;
use dettivo_engine_proto::BackendPreference;
use dettivo_speech::diarize::{
    FALLBACK_MODEL, NEMOTRON_BINARY, NEMOTRON_MAX_SPEAKERS, NEMOTRON_MODEL,
};

/// The model set a pass runs, and why when it is not the configured one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    /// The catalogue id under `diarize`.
    pub(crate) model: String,
    /// Why it replaced the configured set.
    pub(crate) fallback_reason: Option<String>,
}

/// Nemotron runs when it is configured, downloaded and the expected count
/// fits its eight channels; otherwise the sherpa-onnx set runs with the
/// reason (ADR 0073). Any other configured set runs as it is.
pub(crate) fn choose(
    configured: &str,
    expected: Option<u32>,
    ready: impl Fn(&str) -> bool,
) -> Choice {
    let why = match expected {
        _ if configured != NEMOTRON_MODEL => None,
        Some(n) if n > NEMOTRON_MAX_SPEAKERS => Some(format!(
            "{n} speakers are expected and Nemotron tracks at most {NEMOTRON_MAX_SPEAKERS}"
        )),
        _ if !ready(configured) => Some(format!(
            "diarize/{configured} is not downloaded (`{}`)",
            crate::engines::download_command("diarize", configured)
        )),
        _ => None,
    };
    match why {
        None => Choice {
            model: configured.to_string(),
            fallback_reason: None,
        },
        Some(why) => Choice {
            model: FALLBACK_MODEL.to_string(),
            fallback_reason: Some(format!("{why}; diarize/{FALLBACK_MODEL} runs instead")),
        },
    }
}

/// `[engines.diarize] backend` as the load's preference for `binary`: the
/// sherpa-onnx engine has no Vulkan provider, so `vulkan` asks it for
/// `auto`, which a Nemotron pass that fell back to it then gets.
pub(crate) fn preference(backend: DiarizeBackend, binary: &str) -> BackendPreference {
    match backend {
        DiarizeBackend::Auto => BackendPreference::Auto,
        DiarizeBackend::Cpu => BackendPreference::Cpu,
        DiarizeBackend::Cuda => BackendPreference::Cuda,
        DiarizeBackend::Vulkan if binary == NEMOTRON_BINARY => BackendPreference::Vulkan,
        DiarizeBackend::Vulkan => BackendPreference::Auto,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_speech::engines::DIARIZE_BINARY;

    #[test]
    fn nemotron_falls_back_over_eight_speakers_and_when_it_is_missing() {
        let both = |_: &str| true;
        let fits = choose(NEMOTRON_MODEL, Some(8), both);
        assert_eq!(
            (fits.model.as_str(), fits.fallback_reason),
            (NEMOTRON_MODEL, None)
        );
        let nine = choose(NEMOTRON_MODEL, Some(9), both);
        assert_eq!(nine.model, FALLBACK_MODEL);
        let why = nine.fallback_reason.unwrap();
        assert!(
            why.starts_with("9 speakers are expected and Nemotron tracks at most 8"),
            "{why}"
        );
        assert!(
            why.ends_with("diarize/diarization-en runs instead"),
            "{why}"
        );
        let only_sherpa = |id: &str| id == FALLBACK_MODEL;
        let missing = choose(NEMOTRON_MODEL, None, only_sherpa);
        assert_eq!(missing.model, FALLBACK_MODEL);
        let why = missing.fallback_reason.unwrap();
        assert!(
            why.contains("diarize/nemotron-3-diarization is not downloaded")
                && why.contains("--model nemotron-3-diarization"),
            "{why}"
        );
        // Any other set runs as configured, ready or not (the caller refuses).
        let other = choose("diarization", Some(12), |_| false);
        assert_eq!(
            (other.model.as_str(), other.fallback_reason),
            ("diarization", None)
        );
    }

    #[test]
    fn vulkan_reaches_only_the_nemotron_engine() {
        assert_eq!(
            preference(DiarizeBackend::Vulkan, NEMOTRON_BINARY),
            BackendPreference::Vulkan
        );
        assert_eq!(
            preference(DiarizeBackend::Vulkan, DIARIZE_BINARY),
            BackendPreference::Auto
        );
        assert_eq!(
            preference(DiarizeBackend::Cuda, DIARIZE_BINARY),
            BackendPreference::Cuda
        );
        assert_eq!(
            preference(DiarizeBackend::Cpu, NEMOTRON_BINARY),
            BackendPreference::Cpu
        );
    }
}
