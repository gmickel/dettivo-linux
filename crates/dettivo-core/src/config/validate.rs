//! Validation that names the key and the line: a syntax error points at
//! its line, a schema error (unknown key, wrong type, unknown variant)
//! names the dotted key and the line it sits on.

use dettivo_proto::methods::config::ValidationError;

use super::schema::Config;

/// Parses `text` into the typed configuration, or returns the first
/// finding with the key and line named where they are known.
pub fn validate(text: &str) -> Result<Config, ValidationError> {
    let doc = match toml_edit::Document::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(ValidationError {
                key: None,
                line: e.span().map(|s| line_of(text, s.start)),
                message: e.message().to_string(),
            });
        }
    };
    let table: toml::Table = match toml::from_str(text) {
        Ok(t) => t,
        Err(e) => {
            return Err(ValidationError {
                key: None,
                line: e.span().map(|s| line_of(text, s.start)),
                message: e.message().to_string(),
            });
        }
    };
    let de = toml::Value::Table(table);
    match serde_path_to_error::deserialize::<_, Config>(de) {
        Ok(config) => match semantic(&config) {
            None => Ok(config),
            Some((key, message)) => Err(ValidationError {
                line: line_for_key(&doc, text, key),
                key: Some(key.to_string()),
                message,
            }),
        },
        Err(e) => {
            let path = e.path().to_string();
            let message = e.inner().message().to_string();
            let key = key_for(&path);
            let line = key.as_deref().and_then(|k| line_for_key(&doc, text, k));
            Err(ValidationError { key, line, message })
        }
    }
}

/// Findings the types cannot express, checked here so a file, a
/// `config.set` and a reload refuse the same values the pipelines would
/// refuse later: the first offending key and why.
fn semantic(config: &Config) -> Option<(&'static str, String)> {
    if let Some(m) = config.rest.bind_error() {
        return Some(("rest.bind", m));
    }
    let t = &config.transcribe;
    if t.chunk_seconds <= t.overlap_seconds + t.safety_margin_seconds {
        return Some((
            "transcribe.chunk_seconds",
            format!(
                "transcribe.chunk_seconds ({}) must exceed overlap_seconds ({}) plus safety_margin_seconds ({})",
                t.chunk_seconds, t.overlap_seconds, t.safety_margin_seconds
            ),
        ));
    }
    let m = &config.meetings;
    if m.live_tick_ms == 0 || m.live_window_ms == 0 {
        return Some((
            "meetings.live_window_ms",
            "meetings.live_tick_ms and live_window_ms must be positive".into(),
        ));
    }
    if m.live_overlap_ms + m.live_tick_ms > m.live_window_ms {
        return Some((
            "meetings.live_window_ms",
            format!(
                "meetings.live_window_ms ({}) must hold live_overlap_ms ({}) plus live_tick_ms ({})",
                m.live_window_ms, m.live_overlap_ms, m.live_tick_ms
            ),
        ));
    }
    let d = &m.diarization;
    let unit = [
        (
            "dictation.silence_peak_threshold",
            config.dictation.silence_peak_threshold,
        ),
        ("transcribe.silence_rms_floor", t.silence_rms_floor),
        ("meetings.speech_floor_rms", m.speech_floor_rms),
        (
            "meetings.diarization.min_speaker_share",
            d.min_speaker_share,
        ),
    ];
    for (key, value) in unit {
        if !(0.0..=1.0).contains(&value) {
            return Some((key, format!("{key} ({value}) must be between 0 and 1")));
        }
    }
    if !(d.clustering_threshold.is_finite() && d.clustering_threshold >= 0.0) {
        return Some((
            "meetings.diarization.clustering_threshold",
            format!(
                "meetings.diarization.clustering_threshold ({}) must be a finite number of 0 or more",
                d.clustering_threshold
            ),
        ));
    }
    None
}

/// The dotted key a serde error is about. `serde_path_to_error` already
/// appends the field name for "unknown field `x`", so the path is the key;
/// only the root has no key.
fn key_for(path: &str) -> Option<String> {
    if path == "." || path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// The 1-based line a dotted key starts on, when the document has it.
fn line_for_key(doc: &toml_edit::Document<&str>, text: &str, key: &str) -> Option<u32> {
    let mut item = doc.as_item();
    for segment in key.split('.') {
        item = item.get(segment)?;
    }
    let span = item.span()?;
    Some(line_of(text, span.start))
}

/// 1-based line of a byte offset.
pub fn line_of(text: &str, offset: usize) -> u32 {
    let clamped = offset.min(text.len());
    u32::try_from(text[..clamped].matches('\n').count() + 1).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_errors_name_the_line() {
        let err = validate("[daemon]\nlog_level = \n").unwrap_err();
        assert_eq!(err.line, Some(2));
        assert_eq!(err.key, None);
    }

    #[test]
    fn unknown_keys_name_key_and_line() {
        let err =
            validate("[daemon]\nlog_level = \"info\"\n\n[ipc]\ncolour = \"blue\"\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("ipc.colour"));
        assert_eq!(err.line, Some(5));
        assert!(
            err.message.starts_with("unknown field `colour`"),
            "{}",
            err.message
        );
    }

    #[test]
    fn wrong_types_and_variants_name_key_and_line() {
        let err = validate("[daemon]\nshutdown_timeout_ms = \"soon\"\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("daemon.shutdown_timeout_ms"));
        assert_eq!(err.line, Some(2));
        let err = validate("[qa]\nmode = false\n[daemon]\nlog_level = \"loud\"\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("daemon.log_level"));
        assert_eq!(err.line, Some(4));
        assert!(
            err.message.contains("unknown variant `loud`"),
            "{}",
            err.message
        );
    }

    #[test]
    fn a_bind_outside_loopback_names_the_key_and_line() {
        let err = validate("[rest]\nenabled = true\nbind = \"0.0.0.0\"\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("rest.bind"));
        assert_eq!(err.line, Some(3));
        assert!(err.message.contains("loopback"), "{}", err.message);
        for ok in ["127.0.0.1", "127.0.0.53", "::1", "[::1]", "localhost"] {
            assert!(
                validate(&format!("[rest]\nbind = \"{ok}\"\n")).is_ok(),
                "{ok}"
            );
        }
    }

    #[test]
    fn values_the_pipelines_would_refuse_fail_by_key_and_line_before_they_are_installed() {
        // The chunk window has to hold the overlap and the margin.
        let err = validate("[transcribe]\nchunk_seconds = 1\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("transcribe.chunk_seconds"));
        assert_eq!(err.line, Some(2));
        assert!(err.message.contains("must exceed"), "{}", err.message);
        assert!(validate("[transcribe]\nchunk_seconds = 8\n").is_ok());
        // The live window has to hold the overlap and one tick.
        let err = validate("[meetings]\nlive_window_ms = 1000\n").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("meetings.live_window_ms"));
        assert!(err.message.contains("must hold"), "{}", err.message);
        assert!(validate("[meetings]\nlive_window_ms = 1350\n").is_ok());
        assert!(validate("[meetings]\nlive_tick_ms = 0\n").is_err());
        // Shares and floors are unit values; a non-finite one is refused.
        for (text, key) in [
            (
                "[meetings.diarization]\nmin_speaker_share = 1.5\n",
                "meetings.diarization.min_speaker_share",
            ),
            (
                "[meetings.diarization]\nmin_speaker_share = nan\n",
                "meetings.diarization.min_speaker_share",
            ),
            (
                "[meetings.diarization]\nclustering_threshold = inf\n",
                "meetings.diarization.clustering_threshold",
            ),
            (
                "[dictation]\nsilence_peak_threshold = -0.1\n",
                "dictation.silence_peak_threshold",
            ),
            (
                "[transcribe]\nsilence_rms_floor = 2.0\n",
                "transcribe.silence_rms_floor",
            ),
            (
                "[meetings]\nspeech_floor_rms = inf\n",
                "meetings.speech_floor_rms",
            ),
        ] {
            let err = validate(text).unwrap_err();
            assert_eq!(err.key.as_deref(), Some(key), "{text}");
            assert_eq!(err.line, Some(2), "{text}");
        }
        for ok in [
            "[meetings.diarization]\nmin_speaker_share = 1.0\nclustering_threshold = 0.0\n",
            // The retired key still loads and is ignored (ADR 0072).
            "[meetings.diarization]\nmin_coverage = 0.25\n",
            "[dictation]\nsilence_peak_threshold = 1.0\n",
        ] {
            assert!(validate(ok).is_ok(), "{ok}");
        }
        // `config.set` refuses the same value and leaves the file alone.
        let err = super::super::edit::set("", "transcribe.chunk_seconds", &serde_json::json!(6))
            .unwrap_err();
        assert_eq!(err.key.as_deref(), Some("transcribe.chunk_seconds"));
    }

    #[test]
    fn empty_and_default_texts_validate() {
        assert_eq!(validate("").unwrap(), Config::default());
        assert_eq!(
            validate(super::super::schema::DEFAULT_TOML).unwrap(),
            Config::default()
        );
    }
}
