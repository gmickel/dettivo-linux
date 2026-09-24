//! The QA seed (`DETTIVO_E2E_SEED=1`): twelve dictation items across three
//! days and two apps, the macOS harness's expectation, deterministic so
//! drives and goldens see the same rows, plus the four meetings of
//! `seed_meetings`. The newest dictation is the contract's sample
//! dictation and the newest meeting the contract's sample meeting, so the
//! `transcripts.*` and `meetings.*` fixtures replay against the seed.

use serde_json::json;

use crate::item::{DictationItem, ItemStatus, SourceKind, Timings};
use crate::{Store, StoreError};

/// The contract's sample dictation id (`fixtures/transcripts/latest.json`).
pub const CONTRACT_ITEM_ID: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

pub use crate::seed_meetings::{
    CONTRACT_MEETING_ID, NOTES_ONLY_MEETING_ID, PARTIAL_MEETING_ID, RICH_MEETING_ID, meetings,
};

/// The seed row that keeps a take (row 11, the design-call summary): the
/// daemon writes one second of silence for it, so the history drive can
/// re-run an item while the contract sample stays without audio for the
/// `audio_not_retained` fixture.
pub const AUDIO_ITEM_ID: &str = "5eed0000-0000-4000-8000-000000000011";

/// Writes one second of 16 kHz mono silence as a WAV file at `target`,
/// creating its directory.
pub fn write_silence(target: &std::path::Path) -> Result<(), StoreError> {
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let map = |e: hound::Error| StoreError::Io(e.to_string());
    let mut writer = hound::WavWriter::create(target, spec).map_err(map)?;
    for _ in 0..16_000 {
        writer.write_sample(0i16).map_err(map)?;
    }
    writer.finalize().map_err(map)
}

/// The first seeded app: a text editor.
pub const APP_EDITOR: (&str, &str) = ("org.gnome.TextEditor", "Text Editor");
/// The second seeded app: a terminal.
pub const APP_TERMINAL: (&str, &str) = ("com.mitchellh.ghostty", "Ghostty");

/// One seed row: id suffix, created_at, app, duration, raw text, final text.
const ROWS: &[(&str, &str, u8, u64, &str, &str)] = &[
    (
        "01",
        "2026-02-11T09:12:00Z",
        0,
        14_200,
        "morning standup notes comma the api gateway rollout moves to thursday",
        "Morning standup notes, the API gateway rollout moves to Thursday.",
    ),
    (
        "02",
        "2026-02-11T10:40:00Z",
        1,
        6_800,
        "git commit message fix the retry loop in the uploader",
        "Git commit message: fix the retry loop in the uploader.",
    ),
    (
        "03",
        "2026-02-11T14:05:00Z",
        0,
        22_400,
        "draft reply to the naive question about latency period we measured forty milliseconds",
        "Draft reply to the naïve question about latency. We measured forty milliseconds.",
    ),
    (
        "04",
        "2026-02-11T17:30:00Z",
        1,
        9_100,
        "todo review the pull request from mara before the release",
        "Todo: review the pull request from Mara before the release.",
    ),
    (
        "05",
        "2026-02-12T08:55:00Z",
        0,
        31_000,
        "meeting prep comma three points colon budget comma hiring comma the cafe lease",
        "Meeting prep, three points: budget, hiring, the café lease.",
    ),
    (
        "06",
        "2026-02-12T11:20:00Z",
        1,
        4_500,
        "run the benchmark on the vulkan backend",
        "Run the benchmark on the Vulkan backend.",
    ),
    (
        "07",
        "2026-02-12T13:45:00Z",
        0,
        18_700,
        "note to self the search index needs diacritic folding for the german names",
        "Note to self: the search index needs diacritic folding for the German names.",
    ),
    (
        "08",
        "2026-02-12T16:10:00Z",
        1,
        7_300,
        "ssh into the build machine and restart the pipewire service",
        "SSH into the build machine and restart the PipeWire service.",
    ),
    (
        "09",
        "2026-02-13T09:00:00Z",
        0,
        12_600,
        "thanks for the review comma i pushed the fixes for the api rate limiter",
        "Thanks for the review, I pushed the fixes for the API rate limiter.",
    ),
    (
        "10",
        "2026-02-13T11:35:00Z",
        1,
        5_900,
        "cargo test dash dash workspace then package the release",
        "Cargo test --workspace, then package the release.",
    ),
    (
        "11",
        "2026-02-13T14:20:00Z",
        0,
        26_800,
        "summary of the design call colon the bar widget resolves every colour from the theme",
        "Summary of the design call: the bar widget resolves every colour from the theme.",
    ),
];

/// The twelve items, oldest first; the last is the contract sample.
pub fn items() -> Vec<DictationItem> {
    let mut out: Vec<DictationItem> = ROWS
        .iter()
        .map(|(suffix, created, app, duration_ms, raw, text)| {
            let (app_id, app_name) = if *app == 0 { APP_EDITOR } else { APP_TERMINAL };
            let mut item = DictationItem::new(SourceKind::Dictation);
            item.id = format!("5eed0000-0000-4000-8000-0000000000{suffix}");
            item.created_at = (*created).to_string();
            item.updated_at = (*created).to_string();
            item.app_id = app_id.to_string();
            item.app_name = app_name.to_string();
            item.stt_provider = "whisper".into();
            item.stt_model = "large-v3-turbo".into();
            item.language = "en".into();
            item.raw_text = (*raw).to_string();
            item.final_text = (*text).to_string();
            item.duration_ms = *duration_ms;
            item.insertion = Some(insertion(app_id, app_name));
            item.timings = Some(Timings {
                capture_ms: 120,
                transcribe_ms: 640,
                insert_ms: 35,
            });
            item.with_derived_title()
        })
        .collect();
    let mut sample = DictationItem::new(SourceKind::Dictation);
    sample.id = CONTRACT_ITEM_ID.to_string();
    sample.created_at = "2026-02-13T16:00:00Z".into();
    sample.updated_at = "2026-02-13T16:00:00Z".into();
    sample.app_id = APP_EDITOR.0.to_string();
    sample.app_name = APP_EDITOR.1.to_string();
    sample.stt_provider = "whisper".into();
    sample.stt_model = "large-v3-turbo".into();
    sample.language = "en".into();
    sample.raw_text = "the contract sample dictation period thirty seconds of speech".into();
    sample.final_text = "The contract sample dictation. Thirty seconds of speech.".into();
    sample.title = "Dictation".into();
    sample.duration_ms = 30_000;
    sample.status = ItemStatus::Completed;
    sample.insertion = Some(insertion(APP_EDITOR.0, APP_EDITOR.1));
    sample.timings = Some(Timings {
        capture_ms: 90,
        transcribe_ms: 810,
        insert_ms: 20,
    });
    out.push(sample);
    out
}

fn insertion(app_id: &str, app_name: &str) -> serde_json::Value {
    json!({
        "outcome": "inserted",
        "method": "paste",
        "target_app": {"bundle_id": app_id, "name": app_name},
        "context_pack": {
            "status": "Off",
            "reason": "seed",
            "source": {"adapter_id": "seed", "app_class": "generic", "bundle_id": app_id},
            "metrics": {"capture_duration_ms": 0, "payload_bytes": 0, "character_count": 0, "token_budget": 0, "token_estimate": 0}
        },
        "reason": null,
        "backend": {"name": "mock", "latency_ms": 35, "undo_supported": false}
    })
}

/// Inserts every seed item the store does not have yet; returns how many
/// were added, so a restart with the seed on adds nothing.
pub fn apply(store: &Store) -> Result<usize, StoreError> {
    let mut added = 0;
    for item in items() {
        if store.get(&item.id)?.is_none() {
            store.insert(&item)?;
            added += 1;
        }
    }
    for meeting in meetings() {
        if store.get_meeting(&meeting.id)?.is_none() {
            store.insert_meeting(&meeting)?;
            added += 1;
        }
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_items_three_days_two_apps_newest_is_the_contract_sample() {
        let items = items();
        assert_eq!(items.len(), 12);
        let days: std::collections::BTreeSet<&str> =
            items.iter().map(|i| &i.created_at[..10]).collect();
        assert_eq!(days.len(), 3);
        let apps: std::collections::BTreeSet<&str> =
            items.iter().map(|i| i.app_id.as_str()).collect();
        assert_eq!(apps.len(), 2);
        assert_eq!(items.last().unwrap().id, CONTRACT_ITEM_ID);
        assert!(items.windows(2).all(|w| w[0].created_at < w[1].created_at));
        let ids: std::collections::BTreeSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids.len(), 12);
        let store = Store::in_memory().unwrap();
        assert_eq!(apply(&store).unwrap(), 16);
        assert_eq!(apply(&store).unwrap(), 0);
        let meeting = store.get_meeting(CONTRACT_MEETING_ID).unwrap().unwrap();
        assert_eq!(meeting.title, "Weekly sync");
        assert_eq!(meeting.segments.len(), 1);
    }
}
