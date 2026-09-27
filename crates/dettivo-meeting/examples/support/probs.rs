//! The engine's own per-10 ms speaker probabilities as the voice check's
//! evidence (ADR 0076): a unit's score for a speaker is that speaker's
//! mean probability over the unit's frames. Nemotron keeps them (`.npy`,
//! frames by channels, float16 or float32), and its turns name the channel.

use std::collections::HashMap;

use dettivo_engine_proto::SpeakerTurn;
use dettivo_meeting::diarize::renumbered;
use dettivo_meeting::voice_check::{Evidence, Scores, Unit};

/// Frame length of the probabilities.
const FRAME_MS: u64 = 10;

pub struct Probs {
    frames: usize,
    channels: usize,
    values: Vec<f32>,
    /// The product's speaker id of each channel the turns use.
    speakers: Vec<(String, usize)>,
}

/// An IEEE half-precision value as f32.
fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((bits >> 10) & 0x1f);
    let frac = f32::from(bits & 0x3ff);
    sign * match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

impl Probs {
    /// Reads `path` and maps each channel to the speaker id the product's
    /// rule gives the turns that name it.
    pub fn open(path: &str, turns: &[SpeakerTurn]) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if bytes.len() < 10 || &bytes[..6] != b"\x93NUMPY" {
            return Err(format!("{path}: not an .npy file"));
        }
        let (len, at) = if bytes[6] == 1 {
            (usize::from(u16::from_le_bytes([bytes[8], bytes[9]])), 10)
        } else {
            let n = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
            (n as usize, 12)
        };
        let header = std::str::from_utf8(bytes.get(at..at + len).ok_or("short header")?)
            .map_err(|e| e.to_string())?;
        let width = if header.contains("'<f2'") {
            2
        } else if header.contains("'<f4'") {
            4
        } else {
            return Err(format!("{path}: expected little-endian float16 or float32"));
        };
        if header.contains("'fortran_order': True") {
            return Err(format!("{path}: Fortran order"));
        }
        let shape = header
            .split("'shape': (")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .ok_or("no shape")?;
        let dims: Vec<usize> = shape
            .split(',')
            .filter_map(|d| d.trim().parse().ok())
            .collect();
        let [frames, channels] = dims[..] else {
            return Err(format!("{path}: expected two dimensions, got {shape}"));
        };
        let data = &bytes[at + len..];
        if data.len() < frames * channels * width {
            return Err(format!("{path}: truncated"));
        }
        let values = data
            .chunks_exact(width)
            .take(frames * channels)
            .map(|c| match width {
                2 => half(u16::from_le_bytes([c[0], c[1]])),
                _ => f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            })
            .collect();
        let mut sorted: Vec<&SpeakerTurn> = turns.iter().collect();
        sorted.sort_by_key(|t| (t.start_ms, t.end_ms));
        let mut map: HashMap<String, usize> = HashMap::new();
        for (raw, product) in sorted.iter().zip(renumbered(turns)) {
            let channel: usize = raw
                .speaker
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .map_err(|_| format!("turn speaker {} names no channel", raw.speaker))?;
            if channel >= channels {
                return Err(format!("channel {channel} of {channels}"));
            }
            map.insert(product.speaker, channel);
        }
        Ok(Self {
            frames,
            channels,
            values,
            speakers: map.into_iter().collect(),
        })
    }
}

impl Evidence for Probs {
    fn scores(&mut self, units: &[Unit]) -> Vec<Scores> {
        units
            .iter()
            .map(|u| {
                u.speaker.as_ref()?;
                let first = (u.span.0 / FRAME_MS) as usize;
                let last = ((u.span.1 / FRAME_MS) as usize).min(self.frames);
                if last <= first {
                    return None;
                }
                let n = (last - first) as f64;
                Some(
                    self.speakers
                        .iter()
                        .map(|(s, c)| {
                            let sum: f64 = (first..last)
                                .map(|f| f64::from(self.values[f * self.channels + c]))
                                .sum();
                            (s.clone(), sum / n)
                        })
                        .collect(),
                )
            })
            .collect()
    }
}
