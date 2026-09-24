//! The seeded meetings (`DETTIVO_E2E_SEED=1`): the contract's sample and
//! three more across two weeks, so the fixtures, the export goldens and
//! the GUI drives share rows. One is completed with two named speakers
//! (rows of `0006-speakers`, ADR 0035), notes and a ready analysis (the
//! export goldens render it), one is partial, one has notes and no
//! analysis; the contract's sample carries the `You` speaker its
//! diarization pass found.

use dettivo_proto::methods::meetings::{Segment, SegmentSource};
use dettivo_proto::methods::meetings_notes::{ActionItem, Analysis, AnalysisStatus, NotesSource};
use dettivo_proto::methods::speakers::{Diarization, DiarizationStatus, Speaker};

use crate::meetings::{MeetingRow, MeetingStatus};

/// The contract's sample meeting id (`fixtures/meetings/get.json`).
pub const CONTRACT_MEETING_ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
/// The completed meeting with two named speakers, notes and analysis.
pub const RICH_MEETING_ID: &str = "5eed0000-0000-4000-8000-00000000a001";
/// The meeting a restart left partial.
pub const PARTIAL_MEETING_ID: &str = "5eed0000-0000-4000-8000-00000000a002";
/// The completed meeting with notes and no analysis.
pub const NOTES_ONLY_MEETING_ID: &str = "5eed0000-0000-4000-8000-00000000a003";

/// The notes on the rich meeting.
pub const RICH_NOTES: &str = "# Roadmap review\n\n- Budget for Q2 agreed\n- Mara owns the runbook\n- Follow up on the café lease";

/// A segment; the index is assigned by `indexed`.
fn segment(
    start_ms: u64,
    end_ms: u64,
    text: &str,
    polished: &str,
    speaker: Option<&str>,
    source: SegmentSource,
    gap_before_ms: Option<u64>,
) -> Segment {
    Segment {
        index: 0,
        start_ms,
        end_ms,
        text: text.into(),
        speaker: speaker.map(str::to_string),
        speaker_id: speaker.map(|name| speaker_id(name).to_string()),
        speaker_confidence: speaker.map(|_| 1.0),
        source_type: source,
        words: Vec::new(),
        gap_before_ms,
        polished_text: Some(polished.into()),
    }
}

/// The speaker id behind a seeded name: the microphone side is `you`,
/// the first remote speaker `speaker_00`.
fn speaker_id(name: &str) -> &'static str {
    match name {
        "Gordon" | "You" => "you",
        _ => "speaker_00",
    }
}

fn speaker(name: &str, color_index: u32, talk_ms: u64) -> Speaker {
    Speaker {
        speaker_id: speaker_id(name).into(),
        name: name.into(),
        color_index,
        talk_ms,
    }
}

/// A diarization pass that ran to completion at `ran_at`.
fn diarized(ran_at: &str) -> Diarization {
    Diarization {
        status: DiarizationStatus::Ready,
        coverage: Some(1.0),
        engine: Some("dettivo-engine-diarize".into()),
        model: Some("diarize/diarization".into()),
        ran_at: Some(ran_at.into()),
        error: None,
        expected_speakers: None,
        auto: None,
    }
}

/// Numbers the segments in order.
fn indexed(mut segments: Vec<Segment>) -> Vec<Segment> {
    for (i, s) in segments.iter_mut().enumerate() {
        s.index = i as u32;
    }
    segments
}

fn base(id: &str, title: &str, started: &str) -> MeetingRow {
    let mut m = MeetingRow::new();
    m.id = id.into();
    m.created_at = started.into();
    m.updated_at = started.into();
    m.started_at = started.into();
    m.title = title.into();
    m.title_source = "manual".into();
    m.language = "en".into();
    m.stt_provider = "whisper".into();
    m.stt_model = "large-v3-turbo".into();
    m.disclosure_acknowledged_at = Some("2026-01-30T08:00:00Z".into());
    m.system_audio = true;
    m.microphone_takes = 1;
    m
}

/// The contract's sample meeting: `Weekly sync`, thirty minutes, one
/// segment, completed, as `fixtures/meetings/get.json` states it.
pub fn sample() -> MeetingRow {
    let mut m = base(CONTRACT_MEETING_ID, "Weekly sync", "2026-02-13T16:00:00Z");
    m.updated_at = "2026-02-13T16:30:00Z".into();
    m.ended_at = Some("2026-02-13T16:30:00Z".into());
    m.status = MeetingStatus::Completed;
    m.duration_ms = 1_800_000;
    m.disclosure_acknowledged_at = Some("2026-02-13T15:59:00Z".into());
    m.raw_text = "hello".into();
    m.final_text = "Hello.".into();
    m.segments = indexed(vec![segment(
        0,
        1200,
        "Hello.",
        "Hello.",
        Some("You"),
        SegmentSource::Microphone,
        None,
    )]);
    m.speakers = vec![speaker("You", 0, 1200)];
    m.diarization = Some(diarized("2026-02-13T16:31:00Z"));
    m
}

/// The completed meeting with two named speakers, notes and analysis.
pub fn rich() -> MeetingRow {
    let mut m = base(RICH_MEETING_ID, "Roadmap review", "2026-02-02T10:00:00Z");
    m.updated_at = "2026-02-02T10:44:00Z".into();
    m.ended_at = Some("2026-02-02T10:42:00Z".into());
    m.status = MeetingStatus::Completed;
    m.duration_ms = 2_520_000;
    m.segments = indexed(vec![
        segment(
            0,
            3400,
            "um okay so the roadmap for q2",
            "Okay, so the roadmap for Q2.",
            Some("Gordon"),
            SegmentSource::Microphone,
            None,
        ),
        segment(
            3900,
            8100,
            "the budget is agreed we can hire two engineers",
            "The budget is agreed, we can hire two engineers.",
            Some("Mara"),
            SegmentSource::System,
            None,
        ),
        segment(
            8600,
            12000,
            "and the api gateway rollout moves to thursday",
            "And the API gateway rollout moves to Thursday.",
            Some("Mara"),
            SegmentSource::System,
            None,
        ),
        segment(
            14300,
            17900,
            "uh right i will write the runbook by wednesday",
            "Right, I will write the runbook by Wednesday.",
            Some("Gordon"),
            SegmentSource::Microphone,
            Some(2300),
        ),
        segment(
            18200,
            21000,
            "and mara pings ops about the cafe lease",
            "And Mara pings ops about the café lease.",
            Some("Gordon"),
            SegmentSource::Microphone,
            None,
        ),
    ]);
    m.raw_text = m
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    m.final_text = m.polished_join();
    m.speakers = vec![speaker("Gordon", 0, 9800), speaker("Mara", 1, 7600)];
    m.diarization = Some(diarized("2026-02-02T10:43:00Z"));
    m.notes_markdown = RICH_NOTES.into();
    m.notes_source = NotesSource::User;
    m.notes_updated_at = Some("2026-02-02T10:50:00Z".into());
    m.analysis = Some(Analysis {
        summary:
            "The Q2 roadmap review agreed the budget and moved the API gateway rollout to Thursday."
                .into(),
        decisions: vec![
            "Hire two engineers in Q2".into(),
            "Move the API gateway rollout to Thursday".into(),
        ],
        action_items: vec![
            ActionItem {
                text: "Write the rollout runbook".into(),
                owner: Some("Gordon".into()),
                due: Some("Wednesday".into()),
            },
            ActionItem {
                text: "Ping ops about the café lease".into(),
                owner: Some("Mara".into()),
                due: None,
            },
        ],
    });
    m.summary = m
        .analysis
        .as_ref()
        .map(|a| a.summary.clone())
        .unwrap_or_default();
    m.analysis_status = AnalysisStatus::Ready;
    m.analysis_model = Some("qwen3-4b-instruct-2507".into());
    m.analysis_at = Some("2026-02-02T10:44:00Z".into());
    m
}

/// The meeting a daemon restart left partial: a live tail, no notes.
pub fn partial() -> MeetingRow {
    let mut m = base(PARTIAL_MEETING_ID, "Design call", "2026-02-06T14:30:00Z");
    m.status = MeetingStatus::Partial;
    m.is_partial = true;
    m.duration_ms = 540_000;
    m.chunks_completed = 3;
    m.chunks_total = 7;
    m.segments = indexed(vec![
        segment(
            0,
            2800,
            "the bar widget resolves every colour from the theme",
            "The bar widget resolves every colour from the theme.",
            None,
            SegmentSource::Microphone,
            None,
        ),
        segment(
            3100,
            5600,
            "and the pill follows the focused output",
            "And the pill follows the focused output.",
            None,
            SegmentSource::System,
            None,
        ),
    ]);
    m.raw_text = m
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    m.final_text = m.raw_text.clone();
    m
}

/// The completed meeting with notes and no analysis.
pub fn notes_only() -> MeetingRow {
    let mut m = base(
        NOTES_ONLY_MEETING_ID,
        "Vendor intro",
        "2026-02-10T09:15:00Z",
    );
    m.updated_at = "2026-02-10T09:40:00Z".into();
    m.ended_at = Some("2026-02-10T09:36:00Z".into());
    m.status = MeetingStatus::Completed;
    m.duration_ms = 1_260_000;
    m.system_audio = false;
    m.segments = indexed(vec![
        segment(
            0,
            2600,
            "thanks for taking the call",
            "Thanks for taking the call.",
            None,
            SegmentSource::Microphone,
            None,
        ),
        segment(
            3000,
            6900,
            "we measured forty milliseconds of latency on the vulkan backend",
            "We measured forty milliseconds of latency on the Vulkan backend.",
            None,
            SegmentSource::Microphone,
            None,
        ),
    ]);
    m.raw_text = m
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    m.final_text = m.polished_join();
    m.notes_markdown = "Ask for the pricing sheet before the trial.".into();
    m.notes_updated_at = Some("2026-02-10T09:40:00Z".into());
    m
}

/// Every seeded meeting, oldest first; the last is the contract's sample.
pub fn meetings() -> Vec<MeetingRow> {
    vec![rich(), partial(), notes_only(), sample()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_meetings_across_two_weeks_the_newest_is_the_contract_sample() {
        let rows = meetings();
        assert_eq!(rows.len(), 4);
        assert!(rows.windows(2).all(|w| w[0].created_at < w[1].created_at));
        assert_eq!(rows.last().unwrap().id, CONTRACT_MEETING_ID);
        let rich = rich();
        assert_eq!(rich.speaker_labels(), ["Gordon", "Mara"]);
        assert_eq!(rich.speakers[1].speaker_id, "speaker_00");
        assert_eq!(rich.segments[1].speaker_id.as_deref(), Some("speaker_00"));
        assert!(rich.final_text.starts_with("Okay, so the roadmap"));
        assert_eq!(rich.analysis_status, AnalysisStatus::Ready);
        assert!(partial().is_partial && partial().analysis.is_none());
        assert!(notes_only().analysis.is_none() && !notes_only().notes_markdown.is_empty());
    }
}
