//! Migration `0008-analysis-queued` (ADR 0061) recreates the
//! `analysis_status` column of `meetings` with a wider CHECK: it adds a
//! column, copies the statuses, drops the old column and renames. This
//! test builds a store at schema version 7, fills it with meetings that
//! cover every old status and every column a meeting carries, migrates to
//! version 8 and proves nothing changed but the CHECK: every row reads
//! back identical, the search index still finds a title, a notes word and
//! an analysis word, `queued` is now accepted, an unknown status is still
//! refused, and a whole-row update still lands on the moved column.

use dettivo_proto::methods::meetings::{Segment, SegmentSource};
use dettivo_proto::methods::meetings_notes::{ActionItem, Analysis, AnalysisStatus, NotesSource};
use dettivo_proto::methods::speakers::{Diarization, DiarizationStatus, Speaker};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_storage::migrate;
use dettivo_storage::{BUSY_TIMEOUT, Store};

/// The version the migration under test starts from.
const FROM: u32 = 7;

/// The statuses the version 7 CHECK accepts, with a distinct word per
/// row so the search assertions can tell them apart.
const CASES: &[(AnalysisStatus, &str)] = &[
    (AnalysisStatus::None, "alpha"),
    (AnalysisStatus::Running, "bravo"),
    (AnalysisStatus::Ready, "charlie"),
    (AnalysisStatus::Failed, "delta"),
];

fn segment(index: u32, text: &str, speaker: &str) -> Segment {
    Segment {
        index,
        start_ms: u64::from(index) * 1000,
        end_ms: u64::from(index) * 1000 + 900,
        text: text.into(),
        speaker: Some(speaker.into()),
        speaker_id: Some(format!("speaker_0{index}")),
        speaker_confidence: Some(0.9),
        source_type: if index % 2 == 0 {
            SegmentSource::Microphone
        } else {
            SegmentSource::System
        },
        words: Vec::new(),
        gap_before_ms: (index > 0).then_some(100),
        polished_text: Some(format!("{text} (polished)")),
    }
}

/// A meeting with every column filled, keyed on `word` so titles, notes
/// and analyses stay distinct across the rows.
fn meeting(n: usize, status: AnalysisStatus, word: &str) -> MeetingRow {
    let stamp = format!("2026-02-{:02}T16:00:00Z", 10 + n);
    let mut m = MeetingRow::new();
    m.created_at = stamp.clone();
    m.updated_at = stamp.clone();
    m.started_at = stamp.clone();
    m.ended_at = Some(format!("2026-02-{:02}T16:30:00Z", 10 + n));
    m.title = format!("Title {word} sync");
    m.title_source = if n % 2 == 0 { "auto" } else { "manual" }.into();
    m.status = MeetingStatus::Completed;
    m.source_kind = "capture".into();
    m.duration_ms = 1_800_000 + n as u64;
    m.language = "en".into();
    m.stt_provider = "whisper".into();
    m.stt_model = "large-v3-turbo".into();
    m.is_partial = false;
    m.chunks_completed = 3;
    m.chunks_total = 3;
    m.disclosure_acknowledged_at = Some(stamp.clone());
    m.system_audio = true;
    m.microphone_takes = 2;
    m.audio_dir = Some(format!("/var/tmp/meetings/{word}"));
    m.raw_text = format!("raw {word} words about the budget");
    m.final_text = format!("Final {word} words about the budget.");
    m.summary = format!("Summary of {word}.");
    m.segments = vec![
        segment(0, &format!("The {word} budget."), "Mara"),
        segment(1, "Agreed.", "You"),
    ];
    m.error_code = None;
    m.error_message = None;
    m.original_filename = Some(format!("{word}.wav"));
    m.recovery_reason = None;
    m.diarization = Some(Diarization {
        status: DiarizationStatus::Ready,
        coverage: Some(0.87),
        engine: Some("dettivo-engine-diarize".into()),
        model: Some("pyannote-3.1".into()),
        ran_at: Some(stamp.clone()),
        error: None,
        expected_speakers: Some(2),
        auto: Some(true),
    });
    m.speakers = vec![
        Speaker {
            speaker_id: "you".into(),
            name: "You".into(),
            color_index: 0,
            talk_ms: 4000,
        },
        Speaker {
            speaker_id: "speaker_00".into(),
            name: "Mara".into(),
            color_index: 1,
            talk_ms: 9000 + n as u64,
        },
    ];
    m.notes_markdown = format!("# Notes\n\nRemember {word}notes for the follow-up.");
    m.notes_source = if n % 2 == 0 {
        NotesSource::User
    } else {
        NotesSource::Live
    };
    m.notes_updated_at = Some(stamp.clone());
    m.analysis = Some(Analysis {
        summary: format!("The {word}analysis summary."),
        decisions: vec![format!("Decide {word}")],
        action_items: vec![ActionItem {
            text: format!("Follow up on {word}"),
            owner: Some("Mara".into()),
            due: Some("Friday".into()),
        }],
    });
    m.analysis_status = status;
    m.analysis_error = (status == AnalysisStatus::Failed).then(|| format!("{word} timed out"));
    m.analysis_model = Some(format!("mock-{word}"));
    m.analysis_at = Some(stamp);
    m
}

/// The columns of `meetings` in table order.
fn columns(db: &std::path::Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(db).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(meetings)").unwrap();
    stmt.query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn status_of(db: &std::path::Path, id: &str) -> String {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT analysis_status FROM meetings WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn migration_0008_keeps_every_meeting_and_widens_the_status_check() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("dettivo.db");
    let before = &migrate::MIGRATIONS[..FROM as usize];
    assert_eq!(before.last().unwrap().version, FROM);

    // Version 7: every old status stored through the store's own writers.
    let store = Store::open_with(&db, before, BUSY_TIMEOUT).unwrap();
    assert_eq!(store.schema_version().unwrap(), FROM);
    let rows: Vec<MeetingRow> = CASES
        .iter()
        .enumerate()
        .map(|(n, (status, word))| meeting(n, *status, word))
        .collect();
    for row in &rows {
        store.insert_meeting(row).unwrap();
    }
    let snapshot: Vec<MeetingRow> = rows
        .iter()
        .map(|r| store.get_meeting(&r.id).unwrap().unwrap())
        .collect();
    assert_eq!(snapshot, rows, "every field round-trips at version 7");
    let err = store
        .set_meeting_analysis_status(&rows[0].id, AnalysisStatus::Queued, None)
        .unwrap_err();
    assert!(
        err.to_string().contains("CHECK"),
        "version 7 refuses queued: {err}"
    );
    assert_eq!(
        status_of(&db, &rows[0].id),
        "none",
        "the refusal changed nothing"
    );
    let old_columns = columns(&db);
    assert_eq!(
        old_columns.last().map(String::as_str),
        Some("analysis_at"),
        "at version 7 analysis_at is the last column"
    );
    drop(store);

    // The migration.
    let store = Store::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 8);
    assert_eq!(
        store.last_migration().unwrap().as_deref(),
        Some("0008-analysis-queued")
    );
    let mut backup = db.as_os_str().to_owned();
    backup.push(format!(".bak-{FROM}"));
    assert!(
        std::path::PathBuf::from(backup).is_file(),
        "the backup of version 7 was written"
    );
    let new_columns = columns(&db);
    assert_eq!(
        new_columns.last().map(String::as_str),
        Some("analysis_status"),
        "the recreated column sits at the end"
    );
    assert_eq!(
        new_columns
            .iter()
            .filter(|c| c.starts_with("analysis_status"))
            .count(),
        1,
        "exactly one analysis_status column and no analysis_status_next"
    );
    let mut reordered = old_columns.clone();
    reordered.retain(|c| c != "analysis_status");
    reordered.push("analysis_status".into());
    assert_eq!(new_columns, reordered, "only analysis_status moved");

    // Every row reads back identical: the statuses, the notes, the
    // analysis fields, the texts, the segments, the speakers and the
    // diarization block.
    assert_eq!(store.count_meetings().unwrap(), rows.len() as u64);
    for (row, (status, _)) in snapshot.iter().zip(CASES) {
        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert_eq!(&back, row, "{}", row.title);
        assert_eq!(back.analysis_status, *status);
        assert_eq!(status_of(&db, &row.id), status.as_str());
    }

    // The search index (content = 'meetings') still resolves through the
    // moved column: a title word, a notes word and an analysis word.
    for (n, (_, word)) in CASES.iter().enumerate() {
        let id = &rows[n].id;
        let hits = store.search_meetings(word, 10).unwrap();
        assert_eq!(hits.len(), 1, "{word}: one meeting carries it");
        assert_eq!((&hits[0].id, hits[0].matched_field), (id, "title"));
        let hits = store.search_meetings(&format!("{word}notes"), 10).unwrap();
        assert_eq!(hits.len(), 1, "{word}notes");
        assert_eq!((&hits[0].id, hits[0].matched_field), (id, "notes"));
        let hits = store
            .search_meetings(&format!("{word}analysis"), 10)
            .unwrap();
        assert_eq!(hits.len(), 1, "{word}analysis");
        assert_eq!((&hits[0].id, hits[0].matched_field), (id, "analysis"));
    }
    assert_eq!(
        store.search_meetings("budget", 10).unwrap().len(),
        rows.len()
    );

    // The wider CHECK: queued lands, an unknown spelling is still refused.
    let queued = &rows[0].id;
    store
        .set_meeting_analysis_status(queued, AnalysisStatus::Queued, None)
        .unwrap();
    let back = store.get_meeting(queued).unwrap().unwrap();
    assert_eq!(back.analysis_status, AnalysisStatus::Queued);
    assert_eq!(status_of(&db, queued), "queued");
    assert_eq!(back.analysis, rows[0].analysis, "the analysis stays");
    let raw = rusqlite::Connection::open(&db).unwrap();
    let err = raw
        .execute(
            "UPDATE meetings SET analysis_status = 'bogus' WHERE id = ?1",
            [queued],
        )
        .unwrap_err();
    assert!(err.to_string().contains("CHECK"), "{err}");
    assert_eq!(
        status_of(&db, queued),
        "queued",
        "the refusal changed nothing"
    );
    assert_eq!(
        store.fail_stale_meeting_passes("restarted").unwrap(),
        2,
        "the queued and the running analysis settle; the ready diarization blocks stay"
    );
    assert_eq!(status_of(&db, queued), "failed");

    // A whole-row update still addresses the columns by name: the title
    // and the speakers change, the notes and the analysis stay.
    let mut renamed = rows[2].clone();
    renamed.title = "Title renamed after the migration".into();
    renamed.speakers[1].name = "Ada".into();
    renamed.segments.push(segment(2, "One more.", "Ada"));
    store.update_meeting(&renamed).unwrap();
    let back = store.get_meeting(&renamed.id).unwrap().unwrap();
    assert_eq!(back.title, renamed.title);
    assert_eq!(back.speakers, renamed.speakers);
    assert_eq!(back.segments.len(), 3);
    assert_eq!(back.notes_markdown, rows[2].notes_markdown);
    assert_eq!(back.analysis, rows[2].analysis);
    assert_eq!(back.analysis_model, rows[2].analysis_model);
    assert_eq!(back.analysis_at, rows[2].analysis_at);
    assert!(back.updated_at > rows[2].updated_at);
    let hits = store.search_meetings("renamed", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!((&hits[0].id, hits[0].matched_field), (&renamed.id, "title"));
    // The old title word still lives in the transcript, so the row is
    // found, but no longer through the title column.
    let hits = store.search_meetings("charlie", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        (&hits[0].id, hits[0].matched_field),
        (&renamed.id, "transcript"),
        "the old title left the index"
    );

    // The migration is done once: a third open runs nothing.
    drop(store);
    let again = Store::open(&db).unwrap();
    assert_eq!(again.schema_version().unwrap(), 8);
    assert_eq!(again.count_meetings().unwrap(), rows.len() as u64);
}
