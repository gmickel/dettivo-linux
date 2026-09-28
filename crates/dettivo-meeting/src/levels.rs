//! The two tracks' loudness per 10 ms frame, which the two-track speaker
//! rules read (ADR 0075): whether the microphone carries the user's own
//! voice during a line, and whether the system track stayed silent for
//! the whole meeting. The thresholds are the ones the diarization bench's
//! local/remote proxy uses (`scripts/qa/nemotron3/channel_score.py`).

/// Samples per frame at 16 kHz.
const FRAME: usize = 160;
/// Milliseconds per frame.
pub const FRAME_MS: u64 = 10;
/// A frame above this (dBFS) is active speech on its track.
pub const ACTIVE_DB: f32 = -45.0;
/// A microphone frame at least half as loud as the system track (6 dB
/// below it or louder) is the user's own voice rather than the speakers
/// heard through the microphone.
pub const HALF_AS_LOUD_DB: f32 = 6.0;
/// The system track is silent when this share of its frames or fewer is
/// active.
const SILENT_SHARE: f64 = 0.005;
/// Someone speaks on the microphone when this share of its frames or more
/// is active.
const SPEAKING_SHARE: f64 = 0.05;

/// Each track's RMS level in dBFS per 10 ms frame, on the meeting clock.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Levels {
    /// The microphone.
    pub mic: Vec<f32>,
    /// The system track.
    pub system: Vec<f32>,
}

/// RMS dBFS per 10 ms frame of 16 kHz mono samples.
pub fn frame_db(pcm: &[i16]) -> Vec<f32> {
    pcm.chunks_exact(FRAME)
        .map(|frame| {
            let power = frame
                .iter()
                .map(|s| (f64::from(*s) / 32_768.0).powi(2))
                .sum::<f64>()
                / FRAME as f64;
            (10.0 * (power + 1e-18).log10()) as f32
        })
        .collect()
}

impl Levels {
    /// The levels of both tracks' samples.
    pub fn from_pcm(mic: &[i16], system: &[i16]) -> Self {
        Self {
            mic: frame_db(mic),
            system: frame_db(system),
        }
    }

    /// The share of the frames in `[start_ms, end_ms)` where the
    /// microphone is active and at least half as loud as the system track
    /// (a missing system frame counts as silence); 0 for an empty span.
    pub fn voiced_share(&self, start_ms: u64, end_ms: u64) -> f64 {
        let first = (start_ms / FRAME_MS) as usize;
        let last = ((end_ms / FRAME_MS) as usize).min(self.mic.len());
        if last <= first {
            return 0.0;
        }
        let voiced = (first..last)
            .filter(|&i| {
                let mic = self.mic[i];
                let system = self.system.get(i).copied().unwrap_or(f32::NEG_INFINITY);
                mic > ACTIVE_DB && mic >= system - HALF_AS_LOUD_DB
            })
            .count();
        voiced as f64 / (last - first) as f64
    }

    /// True when the system track stayed silent while someone spoke on the
    /// microphone: the condition for a shared (room) microphone.
    pub fn system_silent_while_mic_speaks(&self) -> bool {
        let active = |track: &[f32]| {
            if track.is_empty() {
                return 0.0;
            }
            track.iter().filter(|db| **db > ACTIVE_DB).count() as f64 / track.len() as f64
        };
        active(&self.system) <= SILENT_SHARE && active(&self.mic) >= SPEAKING_SHARE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_voiced_frame_is_active_and_at_least_half_as_loud_as_the_system_track() {
        // Full scale is 0 dBFS; 3277 is about -20 dBFS, 164 about -46.
        let loud = vec![3277_i16; 160];
        let quiet = vec![164_i16; 160];
        let db = frame_db(&loud);
        assert!((db[0] + 20.0).abs() < 0.1, "{db:?}");
        assert_eq!(frame_db(&[0; 159]).len(), 0, "a partial frame is left out");
        let pcm = |frames: &[&Vec<i16>]| frames.iter().flat_map(|f| f.iter().copied()).collect();
        let mic: Vec<i16> = pcm(&[&loud, &loud, &quiet, &loud]);
        // The system track is as loud in frame 1 and far louder in frame 3.
        let system: Vec<i16> = pcm(&[&quiet, &loud, &quiet, &vec![32_000; 160]]);
        let levels = Levels::from_pcm(&mic, &system);
        assert_eq!(levels.voiced_share(0, 40), 0.5, "frames 0 and 1 of 4");
        assert_eq!(levels.voiced_share(20, 40), 0.0);
        assert_eq!(levels.voiced_share(30, 30), 0.0, "empty span");
        assert_eq!(levels.voiced_share(0, 10_000), 0.5, "clamped to the track");
    }

    #[test]
    fn the_system_track_is_silent_only_while_the_microphone_speaks() {
        let speech = vec![3277_i16; 16_000];
        let silence = vec![0_i16; 16_000];
        assert!(Levels::from_pcm(&speech, &silence).system_silent_while_mic_speaks());
        assert!(!Levels::from_pcm(&speech, &speech).system_silent_while_mic_speaks());
        assert!(!Levels::from_pcm(&silence, &silence).system_silent_while_mic_speaks());
        assert!(
            Levels::from_pcm(&speech, &[]).system_silent_while_mic_speaks(),
            "a system track that recorded nothing is silent"
        );
    }
}
