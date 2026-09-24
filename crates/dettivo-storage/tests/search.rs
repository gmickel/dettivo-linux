//! R2: full-text search over the seed (word starts across every text
//! column, diacritics folded, operators escaped, snippets, cursor paging,
//! `score` null, the seed searchable by app and date) and R5's export
//! goldens plus the archive round trip.

use std::path::{Path, PathBuf};

use dettivo_storage::Store;
use dettivo_storage::dictations::ListFilter;
use dettivo_storage::export::{self, Format};
use dettivo_storage::item::{DictationItem, SourceKind};
use dettivo_storage::retention::{Artifacts, Retention};
use dettivo_storage::seed;

fn seeded() -> Store {
    let store = Store::in_memory().unwrap();
    seed::apply(&store).unwrap();
    store
}

fn ids(store: &Store, query: &str) -> Vec<String> {
    store
        .search(query, 20, None)
        .unwrap()
        .hits
        .into_iter()
        .map(|h| h.item.id)
        .collect()
}

#[test]
fn word_starts_match_across_raw_final_title_and_summary() {
    let store = seeded();
    // "api" is in two final texts (and their titles) as a word start.
    let api = ids(&store, "api");
    assert_eq!(api.len(), 2, "{api:?}");
    // "comma" is spoken punctuation: raw text only.
    let raw_only = ids(&store, "comma");
    assert_eq!(raw_only.len(), 3, "{raw_only:?}");
    // A word start: "bench" finds "benchmark", "roll" finds "rollout".
    assert_eq!(ids(&store, "bench").len(), 1);
    assert_eq!(ids(&store, "roll").len(), 1);
    // A summary is searched too.
    let mut item = store.get(seed::CONTRACT_ITEM_ID).unwrap().unwrap();
    item.summary = "Quarterly zebra planning".into();
    store.update(&item).unwrap();
    assert_eq!(
        ids(&store, "zebra"),
        vec![seed::CONTRACT_ITEM_ID.to_string()]
    );
    // Several tokens are ANDed.
    assert_eq!(ids(&store, "api gateway").len(), 1);
    assert_eq!(ids(&store, "api zebra").len(), 0);
    // A title-only word (titles derive from the text, so add one).
    item.title = "Kangaroo".into();
    store.update(&item).unwrap();
    assert_eq!(ids(&store, "kanga").len(), 1);
}

#[test]
fn diacritics_fold_and_operators_are_escaped() {
    let store = seeded();
    assert_eq!(ids(&store, "naive").len(), 1, "naïve matches naive");
    assert_eq!(ids(&store, "naïve").len(), 1);
    assert_eq!(ids(&store, "cafe").len(), 1, "café matches cafe");
    assert_eq!(ids(&store, "CAFÉ").len(), 1);
    // Operators and quotes never reach FTS5 as syntax.
    assert_eq!(ids(&store, "api OR zzz").len(), 0);
    assert_eq!(ids(&store, "\"api\" NOT zebra").len(), 0);
    // `NOT` is a plain token: it prefix-matches "notes" in the API gateway item.
    assert_eq!(ids(&store, "\"api\" NOT gateway").len(), 1);
    assert_eq!(ids(&store, "api*").len(), 2);
    assert_eq!(ids(&store, "(api)").len(), 2);
    assert!(store.search("...", 10, None).is_err());
    let err = store.search(&"x".repeat(2000), 10, None).unwrap_err();
    assert!(err.to_string().contains("1024"), "{err}");
}

#[test]
fn snippets_cursor_paging_and_null_scores() {
    let store = seeded();
    let page = store.search("the", 4, None).unwrap();
    assert_eq!(page.hits.len(), 4);
    assert!(page.hits.iter().all(|h| h.score.is_none()));
    assert!(page.hits.iter().all(|h| !h.snippet.is_empty()));
    let long = store
        .search("design", 10, None)
        .unwrap()
        .hits
        .pop()
        .unwrap();
    assert!(long.snippet.contains("design"), "{}", long.snippet);
    let next = page.next_cursor.clone().expect("more pages");
    let second = store.search("the", 4, Some(&next)).unwrap();
    assert_eq!(second.hits.len(), 4);
    let first_ids: Vec<&str> = page.hits.iter().map(|h| h.item.id.as_str()).collect();
    assert!(
        second
            .hits
            .iter()
            .all(|h| !first_ids.contains(&h.item.id.as_str()))
    );
    let mut cursor = second.next_cursor.clone();
    let mut total = 8;
    while let Some(c) = cursor {
        let p = store.search("the", 4, Some(&c)).unwrap();
        total += p.hits.len();
        cursor = p.next_cursor;
    }
    assert_eq!(total, store.search("the", 100, None).unwrap().hits.len());
    assert!(store.search("the", 4, Some("bogus")).is_err());
}

#[test]
fn the_seed_is_searchable_by_app_and_date() {
    let store = seeded();
    let terminal = store
        .list(&ListFilter {
            limit: 20,
            app_id: Some(seed::APP_TERMINAL.0.into()),
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(terminal.items.len(), 5);
    assert!(
        terminal
            .items
            .iter()
            .all(|i| i.app_name == seed::APP_TERMINAL.1)
    );
    for (day, expected) in [("2026-02-11", 4), ("2026-02-12", 4), ("2026-02-13", 4)] {
        let next = format!(
            "{}{}",
            &day[..8],
            day[8..].parse::<u32>().map(|d| d + 1).unwrap()
        );
        let page = store
            .list(&ListFilter {
                limit: 20,
                since: Some(day.into()),
                until: Some(next),
                ..ListFilter::default()
            })
            .unwrap();
        assert_eq!(page.items.len(), expected, "{day}");
    }
    let hits = store.search("api", 20, None).unwrap().hits;
    assert!(hits.iter().all(|h| h.item.app_id == seed::APP_EDITOR.0));
}

fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

/// The seed rendered in every text format equals the checked-in golden.
/// `UPDATE_GOLDENS=1` rewrites them after a deliberate change.
#[test]
fn seed_exports_match_the_goldens() {
    let store = seeded();
    let items = store.all(&ListFilter::default()).unwrap();
    assert_eq!(items.len(), 12);
    for format in [Format::Json, Format::Md, Format::Txt] {
        let rendered = String::from_utf8(export::render(format, &items)).unwrap();
        let path = goldens().join(format.filename("seed"));
        if std::env::var_os("UPDATE_GOLDENS").is_some() {
            std::fs::write(&path, &rendered).unwrap();
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (UPDATE_GOLDENS=1 writes it)", path.display()));
        assert_eq!(rendered, golden, "{}", path.display());
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&export::render(Format::Json, &items)).unwrap();
    assert_eq!(parsed["items"].as_array().unwrap().len(), 12);
    assert_eq!(parsed["items"][0]["id"], seed::CONTRACT_ITEM_ID);
}

#[test]
fn an_archive_restores_into_an_empty_profile_with_the_same_ids_and_audio() {
    let source = tempfile::tempdir().unwrap();
    let store = seeded();
    let artifacts = Artifacts::new(source.path().join("dictations"));
    let retention = Retention::default();
    let mut item = DictationItem::new(SourceKind::Dictation);
    item.final_text = "With audio.".into();
    let take = source.path().join("take.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&take, spec).unwrap();
    for s in [1i16, -1, 2, -2] {
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
    let audio = artifacts.adopt_audio(&item.id, &take).unwrap();
    item.audio_path = Some(audio.to_string_lossy().into_owned());
    store.insert(&item).unwrap();
    let items = store.all(&ListFilter::default()).unwrap();
    let archive = export::render(Format::Zip, &items);
    let entries = dettivo_storage::zip::read(&archive).unwrap();
    assert_eq!(entries.len(), 2, "items.json plus one audio file");

    let target = tempfile::tempdir().unwrap();
    let empty = Store::in_memory().unwrap();
    let target_artifacts = Artifacts::new(target.path().join("dictations"));
    let restored = export::restore_zip(&empty, &target_artifacts, &retention, &archive).unwrap();
    assert_eq!(restored.len(), 13);
    assert_eq!(empty.count().unwrap(), 13);
    let back = empty.get(&item.id).unwrap().unwrap();
    assert_eq!(back.final_text, "With audio.");
    let restored_audio = PathBuf::from(back.audio_path.unwrap());
    assert!(restored_audio.starts_with(target.path()));
    assert_eq!(
        std::fs::read(&restored_audio).unwrap(),
        std::fs::read(&audio).unwrap()
    );
    assert!(
        target_artifacts
            .dir(&item.id)
            .join("metadata.json")
            .is_file()
    );
    assert!(empty.get(seed::CONTRACT_ITEM_ID).unwrap().is_some());
    // A second restore adds nothing.
    assert_eq!(
        export::restore_zip(&empty, &target_artifacts, &retention, &archive)
            .unwrap()
            .len(),
        0
    );
    assert!(export::restore_zip(&empty, &target_artifacts, &retention, b"junk").is_err());
}
