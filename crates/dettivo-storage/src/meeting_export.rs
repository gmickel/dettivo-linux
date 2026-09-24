//! Meeting export renderers for `transcripts.export` over the meeting
//! kind: the contract's five formats (`txt`, `md`, `json`, `srt`, `vtt`)
//! over one meeting's facts, segments, speakers, notes and analysis (ADR
//! 0036). Every format labels a segment with its speaker (its name after
//! the diarization pass, ADR 0035), or with the side it came from (`you`,
//! `remote`) while the speakers are unnamed; `txt` and `md` mark a gap in
//! the capture before a segment. The segments render polished unless
//! `raw` is asked for. `md` carries the notes (the live editor's text
//! first, then the saved notes) and the analysis; `json` carries the
//! macOS keys plus the speakers and the diarization block. A meeting
//! without segments renders its facts and an empty transcript, never an
//! error. The Markdown document is in `meeting_export_md`.

use dettivo_proto::methods::meetings::{Segment, SegmentSource};
use serde_json::{Map, Value, json};

use crate::meetings::MeetingRow;

/// A meeting export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingFormat {
    /// Plain text.
    Txt,
    /// Markdown.
    Md,
    /// JSON, the macOS keys.
    Json,
    /// SubRip subtitles.
    Srt,
    /// WebVTT.
    Vtt,
}

impl MeetingFormat {
    /// Every format, in the contract's order.
    pub const ALL: [Self; 5] = [Self::Txt, Self::Md, Self::Json, Self::Srt, Self::Vtt];

    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Md => "md",
            Self::Json => "json",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
        }
    }

    /// Parses the wire spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.as_str() == text)
    }

    /// The MIME type of the rendered payload.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Txt => "text/plain",
            Self::Md => "text/markdown",
            Self::Json => "application/json",
            Self::Srt => "application/x-subrip",
            Self::Vtt => "text/vtt",
        }
    }

    /// The suggested file name, the contract's `meeting.<format>`.
    pub fn filename(self) -> String {
        format!("meeting.{}", self.as_str())
    }
}

/// What a render may vary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// The live editor's notes, which take the place of the saved notes.
    pub notes_override: Option<String>,
    /// Render the engine's words rather than the polished segments.
    pub raw: bool,
}

/// `hh:mm:ss,mmm` (SRT) or `hh:mm:ss.mmm` (VTT).
pub(crate) fn stamp(ms: u64, separator: char) -> String {
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000
    )
}

/// The side a segment came from, as the transcript labels it.
pub fn side(segment: &Segment) -> &'static str {
    match segment.source_type {
        SegmentSource::System => "remote",
        SegmentSource::Microphone | SegmentSource::Merged => "you",
    }
}

/// The speaker label of a segment: its speaker's name, or its side while
/// the speakers are unnamed.
pub fn label(segment: &Segment) -> &str {
    match segment.speaker.as_deref() {
        Some(name) if !name.is_empty() => name,
        _ => side(segment),
    }
}

/// The text of a segment under `options`.
pub fn text_of<'a>(segment: &'a Segment, options: &Options) -> &'a str {
    if options.raw {
        return &segment.text;
    }
    segment.polished_text.as_deref().unwrap_or(&segment.text)
}

/// The gap line before a segment that follows missing capture, `[gap 1.2 s]`.
pub(crate) fn gap_line(segment: &Segment) -> Option<String> {
    segment
        .gap_before_ms
        .map(|ms| format!("[gap {:.1} s]", ms as f64 / 1000.0))
}

/// The notes a render carries: the live editor's text, then the saved
/// notes; `None` when the selected value is empty.
pub fn notes_of<'a>(row: &'a MeetingRow, options: &'a Options) -> Option<&'a str> {
    let notes = options
        .notes_override
        .as_deref()
        .unwrap_or(&row.notes_markdown);
    (!notes.trim().is_empty()).then_some(notes)
}

fn subtitle_text(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The transcript text under `options`: the polished join, or the raw one.
pub fn transcript_of(row: &MeetingRow, options: &Options) -> String {
    if row.segments.is_empty() {
        return if options.raw {
            row.raw_text.clone()
        } else {
            row.final_text.clone()
        };
    }
    row.segments
        .iter()
        .map(|s| text_of(s, options).trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Renders one meeting in `format` with the default options.
pub fn render(format: MeetingFormat, row: &MeetingRow) -> Vec<u8> {
    render_with(format, row, &Options::default())
}

/// Renders one meeting in `format` under `options`.
pub fn render_with(format: MeetingFormat, row: &MeetingRow, options: &Options) -> Vec<u8> {
    match format {
        MeetingFormat::Json => json_document(row, options),
        MeetingFormat::Md => crate::meeting_export_md::render(row, options).into_bytes(),
        MeetingFormat::Txt => txt(row, options).into_bytes(),
        MeetingFormat::Srt => {
            let mut out = String::new();
            for (i, s) in row.segments.iter().enumerate() {
                out.push_str(&format!(
                    "{}\n{} --> {}\n{}: {}\n\n",
                    i + 1,
                    stamp(s.start_ms, ','),
                    stamp(s.end_ms, ','),
                    subtitle_text(label(s)),
                    subtitle_text(text_of(s, options))
                ));
            }
            out.into_bytes()
        }
        MeetingFormat::Vtt => {
            let mut out = String::from("WEBVTT\n\n");
            for s in &row.segments {
                out.push_str(&format!(
                    "{} --> {}\n{}: {}\n\n",
                    stamp(s.start_ms, '.'),
                    stamp(s.end_ms, '.'),
                    subtitle_text(label(s)),
                    subtitle_text(text_of(s, options))
                ));
            }
            out.into_bytes()
        }
    }
}

fn txt(row: &MeetingRow, options: &Options) -> String {
    let mut out = format!("{}  {}\n", row.started_at, row.title);
    if row.segments.is_empty() {
        out.push_str(&transcript_of(row, options));
        out.push('\n');
    }
    for s in &row.segments {
        if let Some(gap) = gap_line(s) {
            out.push_str(&gap);
            out.push('\n');
        }
        out.push_str(&format!(
            "[{}] {}: {}\n",
            stamp(s.start_ms, '.'),
            label(s),
            text_of(s, options)
        ));
    }
    out
}

/// One segment as `json` carries it: the contract's segment with `text`
/// under `options`.
fn segment_json(segment: &Segment, options: &Options) -> Value {
    let mut copy = segment.clone();
    copy.text = text_of(segment, options).to_string();
    copy.polished_text = None;
    serde_json::to_value(copy).unwrap_or(Value::Null)
}

/// The macOS keys (FR-G8): `id`, `title`, `started_at`, `ended_at`,
/// `duration_ms`, `status`, `language`, `stt_provider_id`,
/// `stt_model_id`, `transcript`, `segments`, `analysis`, plus `notes`,
/// `speakers` (the contract's speaker objects, ADR 0035) and
/// `diarization` when present.
fn json_document(row: &MeetingRow, options: &Options) -> Vec<u8> {
    let mut doc = Map::new();
    doc.insert("id".into(), json!(row.id));
    doc.insert("title".into(), json!(row.title));
    doc.insert("started_at".into(), json!(row.started_at));
    doc.insert("ended_at".into(), json!(row.ended_at));
    doc.insert("duration_ms".into(), json!(row.duration_ms));
    doc.insert("status".into(), json!(row.status.as_str()));
    doc.insert("language".into(), json!(row.language));
    doc.insert("stt_provider_id".into(), json!(row.stt_provider));
    doc.insert("stt_model_id".into(), json!(row.stt_model));
    doc.insert("transcript".into(), json!(transcript_of(row, options)));
    doc.insert(
        "segments".into(),
        Value::Array(
            row.segments
                .iter()
                .map(|s| segment_json(s, options))
                .collect(),
        ),
    );
    doc.insert("analysis".into(), json!(row.analysis));
    if let Some(notes) = notes_of(row, options) {
        doc.insert("notes".into(), json!(notes));
    }
    if !row.speakers.is_empty() {
        doc.insert("speakers".into(), json!(row.speakers));
    }
    if let Some(diarization) = &row.diarization {
        doc.insert("diarization".into(), json!(diarization));
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(doc)).unwrap_or_default();
    text.push('\n');
    text.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_renders_the_sample_meeting() {
        let row = crate::seed::meetings().pop().unwrap();
        // The seed's segment carries the microphone speaker's name.
        let srt = String::from_utf8(render(MeetingFormat::Srt, &row)).unwrap();
        assert_eq!(srt, "1\n00:00:00,000 --> 00:00:01,200\nYou: Hello.\n\n");
        let vtt = String::from_utf8(render(MeetingFormat::Vtt, &row)).unwrap();
        assert!(vtt.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:01.200\nYou: Hello.\n"));
        let md = String::from_utf8(render(MeetingFormat::Md, &row)).unwrap();
        assert!(md.starts_with("# Weekly sync\n\n- id: `0f8fad5b"));
        assert!(md.contains("- [00:00:00.000] You: Hello.\n"));
        let txt = String::from_utf8(render(MeetingFormat::Txt, &row)).unwrap();
        assert_eq!(
            txt,
            "2026-02-13T16:00:00Z  Weekly sync\n[00:00:00.000] You: Hello.\n"
        );
        let json: serde_json::Value =
            serde_json::from_slice(&render(MeetingFormat::Json, &row)).unwrap();
        assert_eq!(json["speakers"][0]["speaker_id"], "you");
        assert_eq!(json["diarization"]["status"], "ready");
        assert_eq!(json["title"], "Weekly sync");
        // The other side, unnamed, and a gap in the capture are named in
        // every format.
        let mut two = row.clone();
        two.segments.push(Segment {
            index: 1,
            start_ms: 4000,
            end_ms: 5200,
            text: "hi there".into(),
            speaker: None,
            speaker_id: None,
            speaker_confidence: None,
            source_type: SegmentSource::System,
            words: Vec::new(),
            gap_before_ms: Some(2300),
            polished_text: Some("Hi there.".into()),
        });
        let txt = String::from_utf8(render(MeetingFormat::Txt, &two)).unwrap();
        assert!(
            txt.ends_with(
                "[00:00:00.000] You: Hello.\n[gap 2.3 s]\n[00:00:04.000] remote: Hi there.\n"
            ),
            "{txt}"
        );
        let srt = String::from_utf8(render(MeetingFormat::Srt, &two)).unwrap();
        assert!(
            srt.ends_with("2\n00:00:04,000 --> 00:00:05,200\nremote: Hi there.\n\n"),
            "{srt}"
        );
        let raw = String::from_utf8(render_with(
            MeetingFormat::Txt,
            &two,
            &Options {
                raw: true,
                ..Options::default()
            },
        ))
        .unwrap();
        assert!(raw.ends_with("remote: hi there\n"), "{raw}");
        let md = String::from_utf8(render(MeetingFormat::Md, &two)).unwrap();
        assert!(
            md.ends_with("- [gap 2.3 s]\n- [00:00:04.000] remote: Hi there.\n"),
            "{md}"
        );
        let json: serde_json::Value =
            serde_json::from_slice(&render(MeetingFormat::Json, &two)).unwrap();
        assert_eq!(json["segments"][1]["source_type"], "system");
        assert_eq!(json["segments"][1]["gap_before_ms"], 2300);
        assert_eq!(json["segments"][1]["text"], "Hi there.");
        assert_eq!(json["transcript"], "Hello. Hi there.");
        assert!(json.get("notes").is_none());
        let mut bare = row.clone();
        bare.speakers.clear();
        bare.diarization = None;
        let json: serde_json::Value =
            serde_json::from_slice(&render(MeetingFormat::Json, &bare)).unwrap();
        assert!(json.get("speakers").is_none() && json.get("diarization").is_none());
        assert_eq!(MeetingFormat::Vtt.filename(), "meeting.vtt");
        assert_eq!(MeetingFormat::parse("srt"), Some(MeetingFormat::Srt));
        assert_eq!(stamp(3_661_042, ','), "01:01:01,042");
    }
}
