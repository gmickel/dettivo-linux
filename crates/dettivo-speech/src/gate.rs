//! The speech gate a dictation take passes before a recognizer hears it,
//! and the padding the recognizers need. Stage one is the take's level:
//! the loudest 30 ms window, DC removed, under the silence floor is
//! silence, so one click or keyboard thump never counts as speech. Stage
//! two, run only when the level passes, is the Silero voice activity pass:
//! no voice is silence. Without a voice detector, or when it fails, the
//! level decides alone. The gate only decides; it never trims a take.

use dettivo_engine_proto::DetectSpeechParams;

use crate::EngineError;

/// Samples per second of every take.
pub const SAMPLE_RATE: u32 = 16_000;

/// The catalogue entry of the voice activity model (`provider`, `id`).
pub const VAD_MODEL: (&str, &str) = ("whisper", "silero-vad");

/// The voice activity parameters: they keep a whisper and a one-word "Ok".
pub const VAD_PARAMS: DetectSpeechParams = DetectSpeechParams {
    threshold: 0.4,
    min_speech_ms: 160,
};

/// The shortest take a recognizer is given, in milliseconds.
pub const MIN_ENGINE_MS: u32 = 1_500;

/// The silence after a Parakeet take, in milliseconds, so it emits the
/// last word and its full stop.
pub const PARAKEET_TAIL_MS: u32 = 400;

/// Finds speech in a take.
pub trait VoiceDetector: Send + Sync {
    /// Milliseconds of speech in `pcm` under [`VAD_PARAMS`].
    fn speech_ms(&self, pcm: &[i16]) -> Result<u64, EngineError>;
}

/// Which stage found a take silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilenceReason {
    /// The loudest window stayed under the silence floor.
    Level,
    /// The voice activity pass heard no voice.
    NoVoice,
}

impl SilenceReason {
    /// The stage's name in a session's reason (`level`, `voice`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::NoVoice => "voice",
        }
    }
}

/// What the gate decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    /// The take goes to the recognizer.
    Speech,
    /// The take never reaches the recognizer.
    Silence(SilenceReason),
}

/// The loudest 30 ms window of `pcm` in dBFS, DC removed; `None` when the
/// take is shorter than one window.
pub fn loudest_window_dbfs(pcm: &[i16], sample_rate: u32) -> Option<f32> {
    let win = (sample_rate as usize * 30) / 1000;
    if win == 0 || pcm.len() < win {
        return None;
    }
    let mean = pcm.iter().map(|&s| f64::from(s)).sum::<f64>() / pcm.len() as f64;
    let centred = |s: i16| {
        let x = f64::from(s) - mean;
        x * x
    };
    let mut acc: f64 = pcm[..win].iter().map(|&s| centred(s)).sum();
    let mut best = acc;
    for i in win..pcm.len() {
        acc += centred(pcm[i]) - centred(pcm[i - win]);
        best = best.max(acc);
    }
    let rms = (best.max(0.0) / win as f64).sqrt() / f64::from(i16::MAX);
    Some(20.0 * rms.max(1e-9).log10() as f32)
}

/// Judges a take: `pcm` is what was captured before the release, so a
/// sound after it never counts as speech.
pub fn judge(pcm: &[i16], silence_dbfs: f32, vad: Option<&dyn VoiceDetector>) -> GateVerdict {
    match loudest_window_dbfs(pcm, SAMPLE_RATE) {
        Some(db) if db >= silence_dbfs => {}
        _ => return GateVerdict::Silence(SilenceReason::Level),
    }
    let Some(vad) = vad else {
        return GateVerdict::Speech;
    };
    match vad.speech_ms(pcm) {
        Ok(0) => GateVerdict::Silence(SilenceReason::NoVoice),
        Ok(_) => GateVerdict::Speech,
        Err(e) => {
            tracing::warn!(error = %e, "voice activity pass failed; the level stage decides");
            GateVerdict::Speech
        }
    }
}

/// `pcm` padded the way the recognizer needs: Parakeet gets
/// [`PARAKEET_TAIL_MS`] of trailing silence, and every take is at least
/// [`MIN_ENGINE_MS`] long.
pub fn pad_for_engine(mut pcm: Vec<i16>, parakeet: bool, sample_rate: u32) -> Vec<i16> {
    let ms = |n: u32| (sample_rate as usize * n as usize) / 1000;
    if parakeet {
        pcm.resize(pcm.len() + ms(PARAKEET_TAIL_MS), 0);
    }
    if pcm.len() < ms(MIN_ENGINE_MS) {
        pcm.resize(ms(MIN_ENGINE_MS), 0);
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tone at `dbfs` (RMS), `ms` long, riding on `dc`.
    fn tone(dbfs: f32, ms: usize, dc: i16) -> Vec<i16> {
        let amp =
            10f64.powf(f64::from(dbfs) / 20.0) * f64::from(i16::MAX) * std::f64::consts::SQRT_2;
        (0..ms * 16)
            .map(|i| {
                let v = (i as f64 * 2.0 * std::f64::consts::PI * 220.0 / 16_000.0).sin() * amp;
                (v + f64::from(dc)) as i16
            })
            .collect()
    }

    struct Fixed(Result<u64, EngineError>);

    impl VoiceDetector for Fixed {
        fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
            self.0.clone()
        }
    }

    #[test]
    fn the_level_is_the_loudest_window_with_the_dc_removed() {
        let db = loudest_window_dbfs(&tone(-45.0, 500, 0), SAMPLE_RATE).unwrap();
        assert!((db + 45.0).abs() < 0.5, "{db}");
        // A constant offset is no sound at all.
        let offset = loudest_window_dbfs(&vec![3_000i16; 8_000], SAMPLE_RATE).unwrap();
        assert!(offset < -150.0, "{offset}");
        let on_dc = loudest_window_dbfs(&tone(-45.0, 500, 2_000), SAMPLE_RATE).unwrap();
        assert!((on_dc + 45.0).abs() < 0.5, "{on_dc}");
        assert_eq!(loudest_window_dbfs(&[0; 479], SAMPLE_RATE), None);
    }

    /// R1: a click or a thump in a silent room is gated, a quiet word at
    /// -45 dBFS is not. Both transients peak over the old gate's 0.01, so
    /// that gate sent them to the engine.
    #[test]
    fn a_click_is_silence_and_a_quiet_word_is_speech() {
        let mut click = vec![0i16; 16_000];
        click[8_000..8_003].copy_from_slice(&[980, -900, 600]);
        let mut thump = vec![0i16; 16_000];
        for (i, s) in thump[8_000..8_024].iter_mut().enumerate() {
            *s = (655.0 * (1.0 - i as f64 / 24.0)) as i16;
        }
        for take in [click, thump] {
            let peak = take.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!(f64::from(peak) / 32_768.0 > 0.01, "the old gate passed it");
            assert_eq!(
                judge(&take, -50.0, None),
                GateVerdict::Silence(SilenceReason::Level)
            );
        }
        assert_eq!(
            judge(&vec![0i16; 16_000], -50.0, None),
            GateVerdict::Silence(SilenceReason::Level)
        );
        let mut word = vec![0i16; 8_000];
        word.extend(tone(-45.0, 300, 0));
        word.extend(vec![0i16; 8_000]);
        assert_eq!(judge(&word, -50.0, None), GateVerdict::Speech);
        assert_eq!(
            judge(&word[..400], -50.0, None),
            GateVerdict::Silence(SilenceReason::Level),
            "shorter than one window"
        );
    }

    /// R2 and R3: the voice pass runs only after the level passes; no
    /// voice is silence, and a failing pass leaves the level to decide.
    #[test]
    fn the_voice_stage_gates_only_a_take_the_level_passed() {
        let loud = tone(-20.0, 500, 0);
        let none = Fixed(Ok(0));
        let some = Fixed(Ok(320));
        let broken = Fixed(Err(EngineError::Crashed("gone".into())));
        assert_eq!(
            judge(&loud, -50.0, Some(&none)),
            GateVerdict::Silence(SilenceReason::NoVoice)
        );
        assert_eq!(judge(&loud, -50.0, Some(&some)), GateVerdict::Speech);
        assert_eq!(judge(&loud, -50.0, Some(&broken)), GateVerdict::Speech);
        assert_eq!(
            judge(&vec![0i16; 8_000], -50.0, Some(&some)),
            GateVerdict::Silence(SilenceReason::Level)
        );
    }

    /// R4: every take reaches the engine at least 1.5 s long; a Parakeet
    /// take also carries 400 ms of silence after its last sample.
    #[test]
    fn takes_are_padded_for_the_engine() {
        let short = vec![7i16; 3_200];
        let whisper = pad_for_engine(short.clone(), false, SAMPLE_RATE);
        assert_eq!(whisper.len(), 24_000);
        assert_eq!(&whisper[..3_200], &short[..]);
        let long = vec![7i16; 32_000];
        assert_eq!(pad_for_engine(long.clone(), false, SAMPLE_RATE), long);
        let parakeet = pad_for_engine(long.clone(), true, SAMPLE_RATE);
        assert_eq!(parakeet.len(), 32_000 + 6_400);
        assert!(parakeet[32_000..].iter().all(|&s| s == 0));
        assert_eq!(pad_for_engine(short, true, SAMPLE_RATE).len(), 24_000);
    }
}
