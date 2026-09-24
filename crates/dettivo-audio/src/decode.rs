//! Decoding of every import content type (FR-S6 plus the Linux additions)
//! to 16 kHz mono through Symphonia: the declared content type picks the
//! probe hint and the containers it may turn out to be, the decoded frames
//! stream out in blocks (downmixed, resampled with `resample`), and a file
//! whose container is not what its type declares is refused naming both.

use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

use crate::SAMPLE_RATE;
use crate::resample::ToSixteenK;

/// Every content type an import may declare: the probe hint and the
/// container short names Symphonia may detect for it.
const TYPES: &[(&str, &str, &[&str])] = &[
    ("audio/wav", "wav", &["wave"]),
    ("audio/x-wav", "wav", &["wave"]),
    ("audio/mpeg", "mp3", &["mp3", "mp2", "mp1"]),
    ("audio/mp4", "m4a", &["isomp4"]),
    ("audio/m4a", "m4a", &["isomp4"]),
    ("audio/aac", "aac", &["aac", "isomp4"]),
    ("audio/x-caf", "caf", &["caf"]),
    ("audio/caf", "caf", &["caf"]),
    ("audio/aiff", "aiff", &["aiff"]),
    ("audio/flac", "flac", &["flac"]),
    ("audio/ogg", "ogg", &["ogg"]),
];

/// The content types [`Decoder::open`] accepts.
pub fn content_types() -> impl Iterator<Item = &'static str> {
    TYPES.iter().map(|(t, _, _)| *t)
}

/// Why a file could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The declared type is not one an import accepts.
    UnsupportedType(String),
    /// The file could not be opened or read.
    Io(String),
    /// No known container was found in the file.
    NoContainer(String),
    /// The container does not match the declared type; both are named.
    Mismatch {
        /// The declared content type.
        declared: String,
        /// The container Symphonia detected.
        detected: String,
    },
    /// The container holds no audio track, or one no decoder handles.
    Codec(String),
    /// The stream was malformed past what the decoder tolerates.
    Decode(String),
    /// The audio runs past the import limit, named in seconds.
    TooLong {
        /// The limit in force.
        max_seconds: u64,
    },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "{t} is not an import content type"),
            Self::Io(m) => write!(f, "cannot read the audio: {m}"),
            Self::NoContainer(m) => write!(f, "no audio container found: {m}"),
            Self::Mismatch { declared, detected } => write!(
                f,
                "the upload declares {declared} but its container is {detected}"
            ),
            Self::Codec(m) => write!(f, "no decoder for the audio: {m}"),
            Self::Decode(m) => write!(f, "cannot decode the audio: {m}"),
            Self::TooLong { max_seconds } => write!(
                f,
                "the audio is longer than the import limit of {max_seconds} seconds (history.max_import_seconds)"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// What the probe learned before any frame was decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// The declared content type.
    pub content_type: String,
    /// The container Symphonia detected (`wave`, `mp3`, `isomp4`, ...).
    pub container: String,
    /// The codec of the audio track (`pcm_s16le`, `aac`, `mp3`, ...).
    pub codec: String,
    /// The track's sample rate as the container states it.
    pub sample_rate: u32,
    /// The track's channel count as the container states it.
    pub channels: u16,
    /// The container's own duration estimate; a lossy file may say
    /// nothing, and the decoded length always wins.
    pub duration_ms: Option<u64>,
}

/// A streaming decoder: one block of 16 kHz mono samples per packet.
pub struct Decoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    info: Info,
    resampler: Option<ToSixteenK>,
    scratch: Vec<f32>,
    done: bool,
}

fn detected_name(reader: &dyn FormatReader) -> String {
    reader.format_info().short_name.to_string()
}

impl Decoder {
    /// Probes `path` as `content_type` and prepares the audio track.
    pub fn open(path: &Path, content_type: &str) -> Result<Self, DecodeError> {
        let (_, extension, containers) = TYPES
            .iter()
            .find(|(t, _, _)| *t == content_type)
            .ok_or_else(|| DecodeError::UnsupportedType(content_type.to_string()))?;
        let file = std::fs::File::open(path)
            .map_err(|e| DecodeError::Io(format!("{}: {e}", path.display())))?;
        let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        hint.with_extension(extension);
        let reader = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| match e {
                SymphoniaError::IoError(e) => DecodeError::Io(e.to_string()),
                other => DecodeError::NoContainer(other.to_string()),
            })?;
        let detected = detected_name(reader.as_ref());
        if !containers.contains(&detected.as_str()) {
            return Err(DecodeError::Mismatch {
                declared: content_type.to_string(),
                detected,
            });
        }
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| DecodeError::Codec("the container holds no audio track".into()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| DecodeError::Codec("the audio track names no codec".into()))?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| DecodeError::Codec(format!("{e} ({:?})", params.codec)))?;
        let sample_rate = params.sample_rate.unwrap_or(SAMPLE_RATE);
        let duration_ms = track
            .num_frames
            .map(|n| n * 1000 / u64::from(sample_rate.max(1)))
            .or_else(|| {
                let tb = track.time_base?;
                let t = tb.calc_duration(track.duration?)?;
                Some((t.as_secs_f64() * 1000.0) as u64)
            });
        let info = Info {
            content_type: content_type.to_string(),
            container: detected,
            codec: decoder.codec_info().short_name.to_string(),
            sample_rate,
            channels: params
                .channels
                .as_ref()
                .map(|c| c.count() as u16)
                .unwrap_or(1),
            duration_ms,
        };
        Ok(Self {
            track_id: track.id,
            reader,
            decoder,
            info,
            resampler: None,
            scratch: Vec::new(),
            done: false,
        })
    }

    /// What the probe learned.
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The next block of 16 kHz mono samples, `None` after the last.
    pub fn next_block(&mut self) -> Result<Option<Vec<i16>>, DecodeError> {
        if self.done {
            return Ok(None);
        }
        let mut out = Vec::new();
        loop {
            let packet = match self.reader.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => break,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(DecodeError::Decode(e.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let (rate, channels) = match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    if buffer.is_empty() {
                        continue;
                    }
                    let spec = buffer.spec();
                    let rate = spec.rate();
                    let channels = spec.channels().count().max(1);
                    buffer.copy_to_vec_interleaved::<f32>(&mut self.scratch);
                    (rate, channels)
                }
                // A damaged packet is skipped; the rest of the file still
                // decodes, which is what a listener would get too.
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(DecodeError::Decode(e.to_string())),
            };
            let mono: Vec<f32> = self
                .scratch
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect();
            if self.resampler.is_none() && rate != SAMPLE_RATE {
                self.resampler = ToSixteenK::new(rate).map_err(DecodeError::Decode)?;
            }
            match self.resampler.as_mut() {
                Some(r) => r.push(&mono, &mut out).map_err(DecodeError::Decode)?,
                None => out = mono,
            }
            if !out.is_empty() {
                return Ok(Some(to_i16(&out)));
            }
        }
        self.done = true;
        if let Some(r) = self.resampler.as_mut() {
            r.finish(&mut out).map_err(DecodeError::Decode)?;
        }
        Ok((!out.is_empty()).then(|| to_i16(&out)))
    }
}

fn to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|v| (v.clamp(-1.0, 1.0) * 32_767.0).round() as i16)
        .collect()
}

/// Probes `path` without decoding a frame.
pub fn probe(path: &Path, content_type: &str) -> Result<Info, DecodeError> {
    Decoder::open(path, content_type).map(|d| d.info)
}

/// Decodes `path` into a 16 kHz mono 16-bit WAV at `out`, block by block,
/// refusing the file once it runs past `max_seconds`; returns what the
/// probe learned and the samples written.
pub fn decode_to_wav(
    path: &Path,
    content_type: &str,
    out: &Path,
    max_seconds: Option<u64>,
) -> Result<(Info, u64), DecodeError> {
    let mut decoder = Decoder::open(path, content_type)?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(out, spec).map_err(|e| DecodeError::Io(e.to_string()))?;
    let limit = max_seconds.map(|s| s * u64::from(SAMPLE_RATE));
    let mut written = 0u64;
    while let Some(block) = decoder.next_block()? {
        written += block.len() as u64;
        if limit.is_some_and(|l| written > l) {
            drop(writer);
            let _ = std::fs::remove_file(out);
            return Err(DecodeError::TooLong {
                max_seconds: max_seconds.unwrap_or(0),
            });
        }
        for s in block {
            writer
                .write_sample(s)
                .map_err(|e| DecodeError::Io(e.to_string()))?;
        }
    }
    writer
        .finalize()
        .map_err(|e| DecodeError::Io(e.to_string()))?;
    Ok((decoder.info, written))
}
