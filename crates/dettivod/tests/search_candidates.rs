//! Combined ordering must see every FTS match before it limits the candidates.
mod common;

use common::{Daemon, Tree};
use dettivo_storage::{
    Store,
    item::{DictationItem, SourceKind},
    meetings::MeetingRow,
};
use serde_json::{Value, json};

#[test]
fn exact_and_recent_targets_survive_more_than_one_page_of_higher_fts_ranks() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/search-candidate-ordering.json")).unwrap();
    let tree = Tree::new();
    let store = Store::open(&tree.root().join("data/dettivo/dettivo.db")).unwrap();
    for _ in 0..fixture["old_match_count_per_kind"].as_u64().unwrap() {
        let mut d = DictationItem::new(SourceKind::Dictation);
        d.title = "Legacy needle haystack result".into();
        d.raw_text = "needle haystack ".repeat(80);
        d.final_text = d.raw_text.clone();
        d.created_at = fixture["old_created_at"].as_str().unwrap().into();
        store.insert(&d).unwrap();
        let mut m = MeetingRow::new();
        m.title = d.title;
        m.raw_text = d.raw_text;
        m.final_text = d.final_text;
        m.created_at = d.created_at;
        store.insert_meeting(&m).unwrap();
    }
    let mut targets = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let mut d = DictationItem::new(SourceKind::Dictation);
        d.title = case["target_title"].as_str().unwrap().into();
        d.raw_text = format!(
            "{} {}",
            case["query"].as_str().unwrap(),
            "unrelated ".repeat(1000)
        );
        d.final_text = d.raw_text.clone();
        d.created_at = fixture["target_created_at"].as_str().unwrap().into();
        store.insert(&d).unwrap();
        let mut m = MeetingRow::new();
        m.id = d.id.clone();
        m.title = d.title;
        m.raw_text = d.raw_text;
        m.final_text = d.final_text;
        m.created_at = d.created_at;
        store.insert_meeting(&m).unwrap();
        let query = case["query"].as_str().unwrap();
        let limit = fixture["limit"].as_u64().unwrap() as u32;
        assert!(
            store
                .search(query, limit, None)
                .unwrap()
                .hits
                .iter()
                .all(|h| h.item.id != m.id),
            "fixture must put target outside the dictation rank page"
        );
        assert!(
            store
                .search_meetings(query, limit)
                .unwrap()
                .iter()
                .all(|h| h.id != m.id),
            "fixture must put target outside the meeting rank page"
        );
        targets.push(m.id);
    }
    drop(store);
    let daemon = Daemon::spawn(tree, &[]);
    assert_eq!(
        daemon.result(
            "transcripts.search",
            json!({"query": "", "limit": 10, "kinds": []})
        )["items"],
        json!([])
    );
    for (case, target) in fixture["cases"].as_array().unwrap().iter().zip(targets) {
        for kinds in [
            json!(["dictation"]),
            json!(["meeting"]),
            json!(["dictation", "meeting"]),
        ] {
            let result = daemon.result(
                "transcripts.search",
                json!({"query": case["query"], "limit": fixture["limit"], "kinds": kinds}),
            );
            let items = result["items"].as_array().unwrap();
            assert_eq!(items.len(), fixture["limit"].as_u64().unwrap() as usize);
            for (index, kind) in kinds.as_array().unwrap().iter().enumerate() {
                let item = &items[index];
                assert_eq!(
                    item["ref"]["id"], target,
                    "target was truncated under a different order: {result}"
                );
                assert_eq!(item["ref"]["kind"], *kind);
                assert_eq!(item["score"], Value::Null);
                assert!(!item["snippet"].as_str().unwrap().is_empty());
                if kind == "dictation" {
                    assert!(item["item"].is_object());
                    assert!(!item["matches"].as_array().unwrap().is_empty());
                } else {
                    assert_eq!(item["matched_field"], case["matched_field"]);
                }
            }
        }
    }
    daemon.stop();
}
