//! `meetings.rename` against a live daemon over the seed (ADR 0061): the
//! trimmed title lands on the row as `manual`, `meetings.get` and
//! `meetings.search` follow, a rename is allowed on a partial meeting,
//! an empty, control-laden or over-long title is `INVALID_PARAMS` with
//! the message the contract fixes, an unknown meeting is `NOT_FOUND`,
//! and the fixtures replay by name without changing the seed.

mod common;

use common::meetings::fixture;
use common::{Daemon, Tree};
use serde_json::json;

const RICH: &str = "5eed0000-0000-4000-8000-00000000a001";
const PARTIAL: &str = "5eed0000-0000-4000-8000-00000000a002";
const NOTES_ONLY: &str = "5eed0000-0000-4000-8000-00000000a003";

#[test]
fn a_rename_lands_on_the_row_and_the_search_follows() {
    let daemon = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    );
    let renamed = daemon.result(
        "meetings.rename",
        json!({"meeting_id": NOTES_ONLY, "title": "  Quokka budget sync  "}),
    );
    assert_eq!(renamed["title"], "Quokka budget sync", "{renamed}");
    assert_eq!(renamed["title_source"], "manual");
    assert_eq!(renamed["ref"], json!({"kind": "meeting", "id": NOTES_ONLY}));
    let row = daemon.result("meetings.get", json!({"meeting_id": NOTES_ONLY}));
    assert_eq!(row["title"], "Quokka budget sync");
    let listed = daemon.result("meetings.list", json!({"limit": 10, "cursor": null}));
    let item = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["ref"]["id"] == NOTES_ONLY)
        .unwrap();
    assert_eq!(item["title"], "Quokka budget sync", "{item}");
    let hit = daemon.result("meetings.search", json!({"query": "quokka", "limit": 10}));
    assert_eq!(hit["items"][0]["ref"]["id"], NOTES_ONLY, "{hit}");
    assert_eq!(hit["items"][0]["matched_field"], "title");
    // A rename is allowed in every status: the partial meeting too.
    let partial = daemon.result(
        "meetings.rename",
        json!({"meeting_id": PARTIAL, "title": "Design call, cut short"}),
    );
    assert_eq!(partial["title_source"], "manual");
    daemon.stop();
}

#[test]
fn bad_titles_and_unknown_meetings_are_refused() {
    let daemon = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    );
    let empty = daemon.request(
        "meetings.rename",
        json!({"meeting_id": RICH, "title": "   "}),
    );
    assert_eq!(
        empty["error"],
        fixture("meetings/rename.error-invalid-params-empty.json")["error"]["error"]
    );
    let control = daemon.request(
        "meetings.rename",
        json!({"meeting_id": RICH, "title": "Roadmap\nreview"}),
    );
    assert_eq!(control["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(
        control["error"]["message"],
        "title must not contain control characters"
    );
    assert_eq!(control["error"]["data"]["details"]["field"], "title");
    // Control characters are checked before the trim: a title of only a
    // newline is refused as control, not as empty.
    let newline = daemon.request(
        "meetings.rename",
        json!({"meeting_id": RICH, "title": "\n"}),
    );
    assert_eq!(
        newline["error"]["message"],
        "title must not contain control characters"
    );
    let long = daemon.request(
        "meetings.rename",
        json!({"meeting_id": RICH, "title": "é".repeat(201)}),
    );
    assert_eq!(long["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(
        long["error"]["message"],
        "title is 201 characters; the limit is 200"
    );
    let limit = daemon.result(
        "meetings.rename",
        json!({"meeting_id": NOTES_ONLY, "title": "é".repeat(200)}),
    );
    assert_eq!(limit["title"].as_str().unwrap().chars().count(), 200);
    let missing = daemon.request(
        "meetings.rename",
        json!({"meeting_id": "00000000-0000-4000-8000-000000000000", "title": "x"}),
    );
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    let unknown_field = daemon.request(
        "meetings.rename",
        json!({"meeting_id": RICH, "title": "x", "source": "user"}),
    );
    assert_eq!(unknown_field["error"]["data"]["app_code"], "INVALID_PARAMS");
    // The seeded rich meeting keeps its title through the refusals.
    let row = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(row["title"], "Roadmap review");
    daemon.stop();
}

#[test]
fn the_fixture_replays_and_leaves_the_seed_as_it_was() {
    let daemon = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    );
    let f = fixture("meetings/rename.json");
    let before = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(
        daemon.result("meetings.rename", f["request"]["params"].clone()),
        f["response"]["result"]
    );
    let after = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(after["title"], before["title"]);
    assert_eq!(after["transcript"], before["transcript"]);
    daemon.stop();
}
