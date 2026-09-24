//! R3: the seeded rich meeting renders byte-equal to the goldens under
//! `tests/goldens/meeting.{txt,md,srt,vtt,json}` (speakers, timestamps,
//! gap markers, notes and analysis in `md`, the FR-G8 keys plus the
//! speaker objects and the diarization block in `json`),
//! `notes_override` takes precedence over the saved notes in `md` and
//! `json`, and `raw = true` renders the engine's words. Run with
//! `UPDATE_GOLDENS=1` to record the goldens again after a deliberate
//! change.

use std::path::{Path, PathBuf};

use dettivo_storage::meeting_export::{MeetingFormat, Options, render_with};
use dettivo_storage::seed_meetings;

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

fn check(name: &str, actual: &[u8]) {
    let path = goldens_dir().join(name);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        String::from_utf8_lossy(actual),
        String::from_utf8_lossy(&want),
        "{name} differs from the golden"
    );
}

#[test]
fn the_rich_seed_meeting_renders_byte_equal_to_the_goldens() {
    let row = seed_meetings::rich();
    for format in MeetingFormat::ALL {
        let bytes = render_with(format, &row, &Options::default());
        check(&format.filename(), &bytes);
    }
    let json: serde_json::Value =
        serde_json::from_slice(&render_with(MeetingFormat::Json, &row, &Options::default()))
            .unwrap();
    for key in [
        "id",
        "title",
        "started_at",
        "ended_at",
        "duration_ms",
        "status",
        "language",
        "stt_provider_id",
        "stt_model_id",
        "transcript",
        "segments",
        "analysis",
        "notes",
        "speakers",
    ] {
        assert!(json.get(key).is_some(), "json lacks {key}");
    }
    assert_eq!(json["stt_provider_id"], "whisper");
    assert_eq!(json["duration_ms"], 2_520_000);
    assert_eq!(json["speakers"][0]["name"], "Gordon");
    assert_eq!(json["speakers"][1]["speaker_id"], "speaker_00");
    assert_eq!(json["diarization"]["status"], "ready");
    assert_eq!(json["analysis"]["decisions"][0], "Hire two engineers in Q2");
    assert_eq!(json["segments"][3]["gap_before_ms"], 2300);
    assert_eq!(json["segments"][1]["speaker"], "Mara");
}

#[test]
fn the_live_notes_win_and_raw_renders_the_engine_words() {
    let row = seed_meetings::rich();
    let live = Options {
        notes_override: Some("Live draft from the editor.".into()),
        raw: false,
    };
    let md = String::from_utf8(render_with(MeetingFormat::Md, &row, &live)).unwrap();
    assert!(md.contains("## Notes\n\nLive draft from the editor.\n"));
    assert!(!md.contains("Budget for Q2 agreed"));
    assert!(md.contains("## Analysis"));
    let json: serde_json::Value =
        serde_json::from_slice(&render_with(MeetingFormat::Json, &row, &live)).unwrap();
    assert_eq!(json["notes"], "Live draft from the editor.");
    let raw = Options {
        notes_override: None,
        raw: true,
    };
    let txt = String::from_utf8(render_with(MeetingFormat::Txt, &row, &raw)).unwrap();
    assert!(
        txt.contains("[00:00:00.000] Gordon: um okay so the roadmap for q2\n"),
        "{txt}"
    );
    let polished =
        String::from_utf8(render_with(MeetingFormat::Txt, &row, &Options::default())).unwrap();
    assert!(
        polished.contains("[00:00:00.000] Gordon: Okay, so the roadmap for Q2.\n"),
        "{polished}"
    );
    let srt = String::from_utf8(render_with(MeetingFormat::Srt, &row, &raw)).unwrap();
    assert!(srt.contains("Mara: the budget is agreed we can hire two engineers\n"));
    let json: serde_json::Value =
        serde_json::from_slice(&render_with(MeetingFormat::Json, &row, &raw)).unwrap();
    assert_eq!(
        json["transcript"].as_str().unwrap().split(' ').next(),
        Some("um")
    );
}

#[test]
fn notes_override_preserves_absent_empty_whitespace_and_nonempty() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/meeting-export-overrides.json")).unwrap();
    let mut row = seed_meetings::rich();
    row.notes_markdown = fixture["saved_notes"].as_str().unwrap().into();
    for case in fixture["notes"].as_array().unwrap() {
        let override_text = case.get("override").and_then(serde_json::Value::as_str);
        let expected = case["expected"].as_str();
        let options = Options {
            notes_override: override_text.map(str::to_string),
            raw: false,
        };
        let json: serde_json::Value =
            serde_json::from_slice(&render_with(MeetingFormat::Json, &row, &options)).unwrap();
        assert_eq!(
            json.get("notes").and_then(serde_json::Value::as_str),
            expected,
            "{override_text:?}"
        );
        let md = String::from_utf8(render_with(MeetingFormat::Md, &row, &options)).unwrap();
        assert_eq!(
            md.contains("## Notes\n"),
            expected.is_some(),
            "{override_text:?}"
        );
        if let Some(text) = expected {
            assert!(md.contains(text));
        }
    }
}

#[test]
fn subtitle_cues_escape_markup_and_cannot_inject_blocks() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/meeting-export-overrides.json")).unwrap();
    let mut row = seed_meetings::rich();
    row.segments.truncate(1);
    row.segments[0].speaker = Some(fixture["subtitle"]["speaker"].as_str().unwrap().into());
    row.segments[0].text = fixture["subtitle"]["text"].as_str().unwrap().into();
    let original = row.clone();
    for format in [MeetingFormat::Srt, MeetingFormat::Vtt] {
        let rendered = String::from_utf8(render_with(
            format,
            &row,
            &Options {
                raw: true,
                ..Options::default()
            },
        ))
        .unwrap();
        let body = rendered.strip_prefix("WEBVTT\n\n").unwrap_or(&rendered);
        let cues: Vec<_> = body.trim_end().split("\n\n").collect();
        assert_eq!(cues.len(), 1, "{rendered}");
        let lines: Vec<_> = cues[0].lines().collect();
        let timing = usize::from(format == MeetingFormat::Srt);
        assert_eq!(lines.len(), timing + 2, "{rendered}");
        assert_eq!(lines[timing].split(" --> ").count(), 2);
        let payload = lines[timing + 1];
        assert_eq!(
            payload,
            fixture["subtitle"]["expected_payload"].as_str().unwrap()
        );
        assert!(!payload.contains(['<', '>', '\0']));
        assert!(payload.contains("&lt;b&gt;two &amp; three&lt;/b&gt;"));
    }
    assert_eq!(row, original, "rendering leaves stored content unchanged");
}
