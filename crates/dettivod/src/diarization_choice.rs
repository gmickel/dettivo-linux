//! Which model set and backend a speaker pass runs (ADR 0073): Nemotron
//! when it is configured, downloaded and the expected count fits its eight
//! channels, the sherpa-onnx set with the reason otherwise; and which set
//! embeds the voices (the user's, ADR 0075, and each sentence's, ADR 0076).

use std::path::{Path, PathBuf};

use dettivo_core::config::Loaded;
use dettivo_core::config::schema::DiarizeBackend;
use dettivo_engine_proto::BackendPreference;
use dettivo_speech::diarize::{
    DiarizeEngine, FALLBACK_MODEL, NEMOTRON_BINARY, NEMOTRON_MAX_SPEAKERS, NEMOTRON_MODEL,
    binary_for,
};
use dettivo_speech::engines::DIARIZE_BINARY;
use dettivo_speech::models::Readiness;

use crate::daemon::Daemon;

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
/// reason (ADR 0073). Any other configured set runs as it is. `present`
/// answers whether a set is on disk; a quarantined one counts, so the
/// caller refuses it with the verification error instead of falling back.
pub(crate) fn choose(
    configured: &str,
    expected: Option<u32>,
    present: impl Fn(&str) -> bool,
) -> Choice {
    let why = match expected {
        _ if configured != NEMOTRON_MODEL => None,
        Some(n) if n > NEMOTRON_MAX_SPEAKERS => Some(format!(
            "{n} speakers are expected and Nemotron tracks at most {NEMOTRON_MAX_SPEAKERS}"
        )),
        _ if !present(configured) => Some(format!(
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

/// The model set whose embedding model the voice rules use (ADR 0075):
/// the set the pass runs when the sherpa-onnx engine runs it, otherwise
/// the sherpa-onnx fallback set when it is on disk, since Nemotron has no
/// embedding model. The error is why this meeting skips the voiceprint.
pub(crate) fn embedding_set(
    chosen: &str,
    present: impl Fn(&str) -> bool,
) -> Result<String, String> {
    if binary_for(chosen) == DIARIZE_BINARY {
        Ok(chosen.to_string())
    } else if present(FALLBACK_MODEL) {
        Ok(FALLBACK_MODEL.to_string())
    } else {
        Err(format!(
            "diarize/{chosen} has no speaker-embedding model and diarize/{FALLBACK_MODEL} \
             is not downloaded"
        ))
    }
}

/// The engine that embeds the user's voice, and the set it loads, which
/// keys the voiceprint.
pub(crate) struct Voice {
    pub(crate) engine: DiarizeEngine,
    pub(crate) model_id: String,
}

/// The readiness and load path of `diarize/<id>`, when catalogued.
pub(crate) fn find(daemon: &Daemon, id: &str) -> Option<(Readiness, PathBuf)> {
    let store = daemon.models().store();
    store
        .catalogue()
        .find("diarize", id)
        .map(|entry| (store.readiness(entry), store.load_path(entry)))
}

/// Whether `diarize/<id>` is on disk as far as a choice goes: a
/// quarantined set counts, so its load is refused with the verification
/// error instead of replaced. Only a set missing or still arriving is not.
pub(crate) fn present(daemon: &Daemon, id: &str) -> bool {
    matches!(
        find(daemon, id),
        Some((
            Readiness::Ready | Readiness::Unverified | Readiness::Quarantined,
            _
        ))
    )
}

/// The engine binary for `model` over the supervisor, loading `dir`.
pub(crate) fn open(daemon: &Daemon, loaded: &Loaded, model: &str, dir: &Path) -> DiarizeEngine {
    let binary = binary_for(model);
    DiarizeEngine::new(
        daemon.engines().supervisor(),
        binary,
        dir.to_string_lossy().into_owned(),
        Some(loaded.config.engines.diarize.threads),
    )
    .with_backend(preference(loaded.config.engines.diarize.backend, binary))
}

/// The voice engine for a pass that runs `chosen` ([`embedding_set`]),
/// verified like any load; the error is why the voiceprint and the voice
/// check (ADR 0076) are skipped.
pub(crate) fn voice(daemon: &Daemon, loaded: &Loaded, chosen: &str) -> Result<Voice, String> {
    let d = &loaded.config.meetings.diarization;
    if !d.voiceprint && !d.voice_check {
        return Err("[meetings.diarization] voiceprint = false and voice_check = false".into());
    }
    let id = embedding_set(chosen, |id| present(daemon, id))?;
    let Some((_, dir)) = find(daemon, &id) else {
        return Err(format!("diarize/{id} is not catalogued"));
    };
    daemon.models().verifier().ensure(&dir)?;
    Ok(Voice {
        engine: open(daemon, loaded, &id, &dir),
        model_id: id,
    })
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
    fn embeddings_come_from_the_sherpa_set_whichever_set_diarizes() {
        let both = |_: &str| true;
        assert_eq!(embedding_set(NEMOTRON_MODEL, both).unwrap(), FALLBACK_MODEL);
        assert_eq!(
            embedding_set(FALLBACK_MODEL, |_| false).unwrap(),
            FALLBACK_MODEL
        );
        assert_eq!(
            embedding_set("diarization", |_| false).unwrap(),
            "diarization"
        );
        let why = embedding_set(NEMOTRON_MODEL, |id| id == NEMOTRON_MODEL).unwrap_err();
        assert!(
            why.contains("no speaker-embedding model")
                && why.contains("diarize/diarization-en is not downloaded"),
            "{why}"
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
