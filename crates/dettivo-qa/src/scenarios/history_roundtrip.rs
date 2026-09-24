//! History round trip (fn-14 R2): a dictation through the mock microphone
//! lands in the history store, `transcripts.search` finds it, the item
//! survives a restart of the profile's own daemon, and
//! `dictation.reinsert_last` inserts its text again through the mock
//! target, where the read-back matches the first insertion. Drives the
//! socket alone, so it needs no driver and no display.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::driver::Driver;

/// How much of the 11 s fixture is recorded before stop.
const RECORD_FOR: Duration = Duration::from_secs(6);
/// The words the search looks for; the fixture's first sentence.
const QUERY: &str = "fellow americans";

/// The scenario.
pub struct HistoryRoundtrip;

/// What `roundtrip.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    id: String,
    search_hits: usize,
    survived_restart: bool,
    reinserted_id: String,
    text_chars: usize,
    insertions: usize,
}

fn config(repo_root: &std::path::Path) -> String {
    let bin_dir = binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default();
    format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    )
}

/// The lines the mock inserter appended to `inserted.txt`.
fn inserted_lines(ctx: &Context<'_>) -> Vec<String> {
    std::fs::read_to_string(ctx.profile.root.join("state/dettivo/qa/inserted.txt"))
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

fn item_id(result: &Value, what: &str) -> Result<String, String> {
    result["ref"]["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{what} carries no ref.id: {result}"))
}

impl Scenario for HistoryRoundtrip {
    fn id(&self) -> &'static str {
        "history_roundtrip"
    }

    fn summary(&self) -> &'static str {
        "a dictated item is found by transcripts.search, survives a daemon restart and re-inserts"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let (_, wav) = local_model(ctx.profile).ok_or("model missing")?;
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let config = config(ctx.repo_root);
        let env: BTreeMap<String, String> = BTreeMap::from([(
            "DETTIVO_MOCK_MIC".to_string(),
            wav.to_string_lossy().into_owned(),
        )]);
        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        ctx.timings.mark("daemon");

        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        std::thread::sleep(RECORD_FOR);
        let stopped = daemon.call("dictation.stop", json!({}))?;
        let id = item_id(&stopped, "dictation.stop")?;
        if stopped["insertion"]["outcome"] != json!("inserted") {
            return Err(format!(
                "the dictation was not inserted through the mock target: {}",
                stopped["insertion"]
            ));
        }
        let first = inserted_lines(ctx);
        if first.len() != 1 || !first[0].to_lowercase().contains(QUERY) {
            return Err(format!(
                "inserted.txt after the dictation holds {} line(s), expected the fixture's words",
                first.len()
            ));
        }
        ctx.timings.mark("dictation");

        let found = daemon.call(
            "transcripts.search",
            json!({"query": QUERY, "kinds": ["dictation"], "limit": 10}),
        )?;
        let hits = found["items"].as_array().cloned().unwrap_or_default();
        if !hits.iter().any(|h| h["ref"]["id"] == json!(id)) {
            return Err(format!(
                "transcripts.search {QUERY:?} did not return item {id}: {found}"
            ));
        }
        ctx.timings.mark("search");

        // The profile's own daemon restarts; the item must be there when it
        // is back, and reinsert-last must read it from the store.
        daemon.stop();
        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        let item = daemon
            .call(
                "transcripts.get",
                json!({"ref": {"kind": "dictation", "id": id}}),
            )
            .map_err(|e| format!("item {id} is missing after the restart: {e}"))?;
        let text = item["text_polish"]
            .as_str()
            .or_else(|| item["text_raw"].as_str())
            .unwrap_or_default()
            .to_string();
        let reinserted = daemon.call("dictation.reinsert_last", json!({}))?;
        let reinserted_id = item_id(&reinserted, "dictation.reinsert_last")?;
        if reinserted_id != id {
            return Err(format!(
                "dictation.reinsert_last inserted item {reinserted_id}, expected {id}"
            ));
        }
        if reinserted["insertion"]["outcome"] != json!("inserted") {
            return Err(format!(
                "the re-insertion did not go through the mock target: {}",
                reinserted["insertion"]
            ));
        }
        let lines = inserted_lines(ctx);
        if lines.len() != 2 || lines[0] != lines[1] || lines[1] != text.trim_end() {
            return Err(format!(
                "read-back after reinsert: {} line(s), first {:?} characters, last {:?} characters, item {:?} characters",
                lines.len(),
                lines.first().map(|l| l.chars().count()),
                lines.last().map(|l| l.chars().count()),
                text.chars().count()
            ));
        }
        ctx.timings.mark("reinsert");
        daemon.stop();

        let evidence = Evidence {
            id,
            search_hits: hits.len(),
            survived_restart: true,
            reinserted_id,
            text_chars: text.chars().count(),
            insertions: lines.len(),
        };
        std::fs::write(
            ctx.evidence_dir.join("roundtrip.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("roundtrip.json".into());
        Ok(())
    }
}
