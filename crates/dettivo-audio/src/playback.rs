//! Feedback sounds (FR-H6): three short cues synthesized once as 16 kHz
//! WAV files and played through PipeWire's `pw-play`. In QA mode with the
//! mock microphone the cue name is appended to a file instead, so a test
//! can prove which sounds played and which did not.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::SAMPLE_RATE;

/// The three cues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// Capture started.
    Start,
    /// Capture stopped or the session ended.
    Stop,
    /// The session failed.
    Error,
}

impl Cue {
    /// The file stem and the mock log's word.
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Error => "error",
        }
    }
}

/// The cue's samples: a short tone with a fade in and out (start rises,
/// stop falls, error is two low tones).
pub fn synthesize(cue: Cue) -> Vec<i16> {
    let segments: &[(f32, u32)] = match cue {
        Cue::Start => &[(660.0, 70), (880.0, 90)],
        Cue::Stop => &[(880.0, 70), (660.0, 90)],
        Cue::Error => &[(330.0, 90), (0.0, 40), (330.0, 90)],
    };
    let mut out = Vec::new();
    for (freq, ms) in segments {
        let n = (SAMPLE_RATE * ms / 1000) as usize;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let fade = (i.min(n - i) as f32 / (SAMPLE_RATE as f32 * 0.008)).min(1.0);
            let v = if *freq == 0.0 {
                0.0
            } else {
                (t * freq * std::f32::consts::TAU).sin() * 0.35 * fade
            };
            out.push((v * i16::MAX as f32) as i16);
        }
    }
    out
}

/// Writes 16 kHz mono 16-bit samples as a WAV file.
pub fn write_wav(path: &Path, samples: &[i16]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(std::io::Error::other)?;
    for s in samples {
        writer.write_sample(*s).map_err(std::io::Error::other)?;
    }
    writer.finalize().map_err(std::io::Error::other)
}

enum Mode {
    /// `pw-play` on the cue files under the directory.
    PwPlay(PathBuf),
    /// Append the cue name to the file.
    Mock(PathBuf),
    /// Nothing can play; the reason.
    Unavailable(String),
}

/// Plays cues.
pub struct Player {
    mode: Mode,
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
        .unwrap_or(false)
}

impl Player {
    /// A player writing the cue files under `dir` and playing them with
    /// `pw-play`; `mock` names the QA log file instead.
    pub fn new(dir: PathBuf, mock: Option<PathBuf>) -> Self {
        let mode = match mock {
            Some(file) => Mode::Mock(file),
            None if on_path("pw-play") => Mode::PwPlay(dir),
            None => Mode::Unavailable("pw-play is not on PATH (pacman -S pipewire)".into()),
        };
        Self { mode }
    }

    /// Why nothing can play, when so.
    pub fn unavailable_reason(&self) -> Option<&str> {
        match &self.mode {
            Mode::Unavailable(r) => Some(r),
            _ => None,
        }
    }

    /// Plays a cue without waiting for it to end.
    pub fn play(&self, cue: Cue) -> Result<(), String> {
        match &self.mode {
            Mode::Mock(file) => {
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)
                    .map_err(|e| e.to_string())?;
                writeln!(f, "{}", cue.name()).map_err(|e| e.to_string())
            }
            Mode::PwPlay(dir) => {
                let path = dir.join(format!("{}.wav", cue.name()));
                if !path.is_file() {
                    write_wav(&path, &synthesize(cue))
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                }
                Command::new("pw-play")
                    .arg(&path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| format!("pw-play: {e}"))
            }
            Mode::Unavailable(r) => Err(r.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_are_short_and_written_as_wav() {
        for cue in [Cue::Start, Cue::Stop, Cue::Error] {
            let samples = synthesize(cue);
            let ms = samples.len() as u32 * 1000 / SAMPLE_RATE;
            assert!((100..=300).contains(&ms), "{cue:?}: {ms} ms");
            assert!(samples.iter().any(|s| s.abs() > 1000), "{cue:?} is silent");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sounds/start.wav");
        write_wav(&path, &synthesize(Cue::Start)).unwrap();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert_eq!(reader.spec().channels, 1);
    }

    #[test]
    fn the_mock_player_logs_cue_names_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("qa/sounds.txt");
        let player = Player::new(dir.path().join("cache"), Some(log.clone()));
        player.play(Cue::Start).unwrap();
        player.play(Cue::Error).unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "start\nerror\n");
        assert!(player.unavailable_reason().is_none());
    }
}
