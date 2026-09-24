//! The meeting repository under test: insert, list by keyset, the stale
//! rows a restart promotes, update, read back (the notes and analysis
//! with the row), delete, and the stored spellings of every status.

use super::*;
use dettivo_proto::methods::meetings_notes::{Analysis, AnalysisStatus};

#[test]
fn meetings_round_trip_list_and_delete() {
    let store = Store::in_memory().unwrap();
    let mut a = MeetingRow::new();
    a.title = "Weekly sync".into();
    a.created_at = "2026-02-13T16:00:00Z".into();
    a.started_at = a.created_at.clone();
    a.notes_markdown = "# Notes\n\nBudget agreed.".into();
    a.analysis = Some(Analysis {
        summary: "Budget agreed for Q2.".into(),
        decisions: vec!["Hire two engineers".into()],
        action_items: Vec::new(),
    });
    a.analysis_status = AnalysisStatus::Ready;
    store.insert_meeting(&a).unwrap();
    let mut b = MeetingRow::new();
    b.created_at = "2026-02-14T16:00:00Z".into();
    b.status = MeetingStatus::Partial;
    b.is_partial = true;
    store.insert_meeting(&b).unwrap();
    assert_eq!(store.count_meetings().unwrap(), 2);
    assert_eq!(store.latest_meeting().unwrap().unwrap().id, b.id);
    let page = store
        .list_meetings(&ListFilter {
            limit: 1,
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(page.items[0].id, b.id);
    let next = store
        .list_meetings(&ListFilter {
            limit: 1,
            cursor: page.next_cursor,
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(next.items[0].id, a.id);
    assert!(next.next_cursor.is_none());
    let stale = store
        .meetings_with_status(&[MeetingStatus::Partial])
        .unwrap();
    assert_eq!(stale.len(), 1);
    assert!(stale[0].is_partial);
    a.status = MeetingStatus::Stopped;
    a.duration_ms = 1500;
    store.update_meeting(&a).unwrap();
    let back = store.get_meeting(&a.id).unwrap().unwrap();
    assert_eq!(back.status, MeetingStatus::Stopped);
    assert_eq!(back.duration_seconds(), 2);
    assert_eq!(back.notes_markdown, a.notes_markdown);
    assert_eq!(back.analysis, a.analysis);
    assert_eq!(back.analysis_status, AnalysisStatus::Ready);
    assert_eq!(store.delete_meeting_row(&b.id).unwrap().id, b.id);
    assert!(store.delete_meeting_row(&b.id).is_err());
    for s in ["recording", "stopping", "stopped", "partial", "cancelled"] {
        assert_eq!(MeetingStatus::parse(s).unwrap().as_str(), s);
    }
}

fn speaker(id: &str, name: &str) -> dettivo_proto::methods::speakers::Speaker {
    dettivo_proto::methods::speakers::Speaker {
        speaker_id: id.into(),
        name: name.into(),
        color_index: 0,
        talk_ms: 1000,
    }
}

/// daemon/F1, meetings/F1 (fn-43): the capture worker keeps the row it
/// started with; a checkpoint written from it must not erase the notes
/// and the analysis saved meanwhile through their own setters.
#[test]
fn a_whole_row_update_keeps_the_notes_and_the_analysis_saved_meanwhile() {
    let store = Store::in_memory().unwrap();
    let mut row = MeetingRow::new();
    store.insert_meeting(&row).unwrap();
    store
        .set_meeting_notes(
            &row.id,
            "# Notes\n\nHire two.",
            dettivo_proto::methods::meetings_notes::NotesSource::Live,
        )
        .unwrap();
    let analysis = Analysis {
        summary: "Budget agreed.".into(),
        decisions: vec!["Hire two".into()],
        action_items: Vec::new(),
    };
    store
        .set_meeting_analysis(&row.id, &analysis, "mock", "2026-02-13T16:31:00Z")
        .unwrap();
    // The checkpoint: the worker's row still carries empty notes.
    row.duration_ms = 5000;
    row.microphone_takes = 1;
    store.update_meeting(&row).unwrap();
    let back = store.get_meeting(&row.id).unwrap().unwrap();
    assert_eq!(back.duration_ms, 5000);
    assert_eq!(
        back.notes_markdown, "# Notes\n\nHire two.",
        "the notes survive"
    );
    assert_eq!(back.analysis, Some(analysis), "the analysis survives");
    assert_eq!(back.analysis_status, AnalysisStatus::Ready);
    assert_eq!(back.summary, "Budget agreed.");
    assert_eq!(
        store.search_meetings("hire", 5).unwrap().len(),
        1,
        "the search hit survives"
    );
}

/// meetings/F12 (fn-43): the row and its speaker table change together;
/// an insert that fails leaves both as they were.
#[test]
fn the_row_and_its_speakers_change_together_or_not_at_all() {
    let store = Store::in_memory().unwrap();
    let mut m = MeetingRow::new();
    m.title = "Before".into();
    m.speakers = vec![speaker("speaker_00", "Ada")];
    store.insert_meeting(&m).unwrap();
    m.title = "After".into();
    // The second speaker repeats the id: the primary key refuses it after
    // the first insert.
    m.speakers = vec![
        speaker("speaker_01", "Grace"),
        speaker("speaker_01", "Twice"),
    ];
    assert!(store.update_meeting(&m).is_err());
    let back = store.get_meeting(&m.id).unwrap().unwrap();
    assert_eq!(back.title, "Before", "the row update rolled back");
    assert_eq!(back.speakers.len(), 1, "the speakers rolled back");
    assert_eq!(back.speakers[0].name, "Ada");
    let mut fresh = MeetingRow::new();
    fresh.speakers = vec![
        speaker("speaker_01", "Grace"),
        speaker("speaker_01", "Twice"),
    ];
    assert!(store.insert_meeting(&fresh).is_err());
    assert!(
        store.get_meeting(&fresh.id).unwrap().is_none(),
        "the insert rolled back"
    );
}

/// meetings/F16 (fn-43): a NULL column is absent data; a column whose
/// JSON does not parse is an error naming the meeting and the column,
/// and an older payload without the newer optional keys still reads.
#[test]
fn unreadable_json_is_an_error_and_null_is_absent() {
    use dettivo_proto::methods::meetings::{Segment, SegmentSource};
    let store = Store::in_memory().unwrap();
    let m = MeetingRow::new();
    store.insert_meeting(&m).unwrap();
    let mut old = serde_json::to_value(Segment {
        index: 0,
        start_ms: 0,
        end_ms: 900,
        text: "The budget.".into(),
        speaker: None,
        speaker_id: None,
        speaker_confidence: None,
        source_type: SegmentSource::Microphone,
        words: Vec::new(),
        gap_before_ms: None,
        polished_text: None,
    })
    .unwrap();
    for newer in [
        "speaker_id",
        "speaker_confidence",
        "words",
        "gap_before_ms",
        "polished_text",
    ] {
        old.as_object_mut().unwrap().remove(newer);
    }
    store
        .conn()
        .execute(
            "UPDATE meetings SET segments = ?2, analysis = NULL, diarization = NULL WHERE id = ?1",
            rusqlite::params![m.id, serde_json::json!([old]).to_string()],
        )
        .unwrap();
    let back = store.get_meeting(&m.id).unwrap().unwrap();
    assert_eq!(back.segments.len(), 1, "an older payload still reads");
    assert!(back.analysis.is_none() && back.diarization.is_none());
    for column in ["segments", "diarization", "analysis"] {
        store
            .conn()
            .execute(
                &format!("UPDATE meetings SET {column} = 'not json' WHERE id = ?1"),
                rusqlite::params![m.id],
            )
            .unwrap();
        let err = store.get_meeting(&m.id).unwrap_err().to_string();
        assert!(
            err.contains(&m.id) && err.contains(column),
            "{column}: {err}"
        );
        assert_eq!(
            store.list_meetings(&ListFilter::default()).unwrap().items[0].id,
            m.id
        );
        store
            .conn()
            .execute(
                &format!("UPDATE meetings SET {column} = NULL WHERE id = ?1"),
                rusqlite::params![m.id],
            )
            .unwrap();
    }
    assert!(
        store
            .get_meeting(&m.id)
            .unwrap()
            .unwrap()
            .segments
            .is_empty()
    );
}

#[test]
fn archive_summaries_do_not_decode_unused_payloads() {
    let store = Store::in_memory().unwrap();
    let mut completed = MeetingRow::new();
    completed.status = MeetingStatus::Completed;
    completed.title = "Archive needle".into();
    completed.final_text = "long transcript ".repeat(4096);
    completed.notes_markdown = " ".repeat(dettivo_proto::methods::meetings_notes::MAX_NOTES_BYTES);
    store.insert_meeting(&completed).unwrap();
    for _ in 0..128 {
        let archived = MeetingRow {
            status: MeetingStatus::Completed,
            final_text: "archived transcript ".repeat(1024),
            ..MeetingRow::new()
        };
        store.insert_meeting(&archived).unwrap();
    }
    let partial = MeetingRow {
        status: MeetingStatus::Partial,
        ..MeetingRow::new()
    };
    store.insert_meeting(&partial).unwrap();
    store
        .conn()
        .execute(
            "UPDATE meetings SET analysis = 'not json' WHERE id = ?1",
            [&completed.id],
        )
        .unwrap();
    assert!(
        store.get_meeting(&completed.id).is_err(),
        "detail still rejects corruption"
    );
    let rows = store
        .meetings_with_status(&[MeetingStatus::Partial])
        .expect("unrelated archived payload must not break recovery");
    assert_eq!(
        rows.iter().map(|r| &r.id).collect::<Vec<_>>(),
        [&partial.id]
    );
    assert_eq!(
        store.search_meetings("needle", 5).unwrap()[0].id,
        completed.id
    );
    let page = store
        .list_meetings(&ListFilter {
            limit: 1000,
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(page.items.len(), 130);
    assert!(
        !page
            .items
            .iter()
            .find(|r| r.id == completed.id)
            .unwrap()
            .has_notes
    );
    let progress = store.meeting_progress(&completed.id).unwrap().unwrap();
    assert_eq!(progress.capture.id, completed.id);
    assert_eq!((progress.segment_count, progress.last_end_ms), (0, 0));
    let captures = store
        .meeting_captures_with_status(&[MeetingStatus::Partial])
        .unwrap();
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].id, partial.id);
    let hits = store
        .search_combined("needle", 5, &[dettivo_proto::runtime::RefKind::Meeting])
        .unwrap();
    assert!(
        matches!(&hits[0], crate::combined_search::CombinedHit::Meeting(h) if h.id == completed.id)
    );
    for (notes, has_notes) in [
        ("\t\n\u{a0}\u{2003}", false),
        ("\u{2003}note\n", true),
        ("\0", true),
    ] {
        store
            .conn()
            .execute(
                "UPDATE meetings SET notes_markdown = ?1 WHERE id = ?2",
                rusqlite::params![notes, completed.id],
            )
            .unwrap();
        let page = store
            .list_meetings(&ListFilter {
                limit: 1000,
                ..ListFilter::default()
            })
            .unwrap();
        assert_eq!(
            page.items
                .iter()
                .find(|r| r.id == completed.id)
                .unwrap()
                .has_notes,
            has_notes
        );
    }
}

#[test]
fn polling_projects_segment_counts_and_rejects_unreadable_counts() {
    let store = Store::in_memory().unwrap();
    let row = MeetingRow::new();
    store.insert_meeting(&row).unwrap();
    for (segments, expected) in [
        ("[{\"end_ms\":700},{\"end_ms\":1500}]", Some((2, 1500))),
        ("[]", Some((0, 0))),
        ("not json", None),
        ("{}", None),
        ("[{}]", None),
    ] {
        store
            .conn()
            .execute(
                "UPDATE meetings SET segments = ?1 WHERE id = ?2",
                rusqlite::params![segments, row.id],
            )
            .unwrap();
        let read = store.meeting_progress(&row.id);
        match expected {
            Some(expected) => {
                let progress = read.unwrap().unwrap();
                assert_eq!((progress.segment_count, progress.last_end_ms), expected);
            }
            None => assert!(read.is_err(), "invalid projected fields: {segments}"),
        }
    }
}
