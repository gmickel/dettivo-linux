//! The fixtures the contract replay skips because their answer needs
//! state an earlier request built; each names why in the report, and the
//! test below holds the replay to its claim: every skipped fixture's
//! method is exercised by a daemon integration test under
//! `crates/dettivod/tests/`, or the crate's tests fail naming the method.

/// Fixtures whose answer depends on state an earlier request on the same
/// connection built (an active dictation session, a subscription, a
/// previous insertion): one line on a fresh connection cannot reach that
/// state, so the daemon's integration tests cover them and the replay
/// reports them as skipped, never as passed.
pub const STATEFUL: &[(&str, &str)] = &[
    (
        "dictation/toggle.json",
        "starts or stops a capture and needs the selected speech model; concurrent default and override toggles covered by toggles_serialize_and_preserve_configured_mode",
    ),
    ("dictation/cancel.json", "needs an active dictation session"),
    (
        "dictation/start.json",
        "needs the speech model the config fixtures leave selected",
    ),
    (
        "polish/rules.create.json",
        "the rule id is minted by the daemon",
    ),
    (
        "polish/rules.update.json",
        "needs the rule polish/rules.create.json made",
    ),
    (
        "polish/rules.delete.json",
        "removes the seeded rule the other polish fixtures read",
    ),
    ("llm/models.status.json", "the models dir is per machine"),
    ("llm/models.download.json", "downloads a real model"),
    ("llm/models.cancel.json", "needs a running download"),
    ("llm/models.delete.json", "removes a model from disk"),
    ("llm/engine.status.json", "the engine is per machine"),
    ("dictation/status.json", "needs an active dictation session"),
    ("dictation/stop.json", "needs an active dictation session"),
    ("dictation/reinsert_last.json", "needs a previous insertion"),
    (
        "dictation/reinsert_last.error-not-found.json",
        "needs an empty history; the replay daemon carries the seed",
    ),
    (
        "dictation/start.error-conflict-meeting-active.json",
        "needs an active meeting session",
    ),
    (
        "meetings/start.json",
        "mints a meeting id and needs a capture source",
    ),
    (
        "meetings/start.error-conflict-engine-without-timestamps.json",
        "needs a provider without meeting timestamps selected",
    ),
    ("meetings/stop.json", "needs an active meeting session"),
    ("meetings/cancel.json", "needs an active meeting session"),
    ("meetings/status.json", "needs an active meeting session"),
    (
        "meetings/delete.json",
        "removes the seeded meeting the other meeting fixtures read",
    ),
    ("meetings/recover.json", "needs a partial meeting"),
    ("meetings/discard.json", "needs a partial meeting"),
    (
        "meetings/disclosure.acknowledge.json",
        "records the acknowledgement the start fixture expects absent",
    ),
    (
        "meetings/diarize.json",
        "the diarization model set is per machine",
    ),
    (
        "meetings/diarize.error-not-found-model-missing.json",
        "the diarization model set is per machine",
    ),
    (
        "meetings/diarize.error-conflict-not-completed.json",
        "needs a recording meeting",
    ),
    (
        "meetings/diarize.error-conflict-running.json",
        "needs a running diarization pass",
    ),
    (
        "meetings/speakers.rename.json",
        "renames the seeded speaker the transcript fixtures read",
    ),
    (
        "meetings/speakers.suggest.json",
        "needs the name the rename fixture gives",
    ),
    (
        "meetings/notes.set.json",
        "updated_at is the daemon's clock",
    ),
    (
        "meetings/analyze.json",
        "needs a language model provider and mints a job id",
    ),
    (
        "transcripts/import.json",
        "needs an upload begun on this daemon",
    ),
    (
        "transcripts/export.json",
        "needs a download transfer begun on this daemon",
    ),
    (
        "events/unsubscribe.json",
        "needs a subscription on the same connection",
    ),
    (
        "transcripts/rerun.json",
        "needs an item with retained audio",
    ),
    ("transcripts/cancel.json", "needs a running import job"),
    (
        "transcripts/import.error-conflict-already-in-progress.json",
        "needs a running import",
    ),
    (
        "transfer/chunk.json",
        "needs an upload begun on this daemon",
    ),
    (
        "transfer/chunk.error-rate-limited.json",
        "needs an upload begun on this daemon",
    ),
    (
        "transfer/commit.json",
        "needs an upload begun on this daemon",
    ),
    (
        "transfer/cancel.json",
        "needs an upload begun on this daemon",
    ),
    ("transfer/pull.json", "needs an export bound to a download"),
];

#[cfg(test)]
mod tests {
    use super::STATEFUL;
    use std::path::Path;

    /// The method a fixture replays, from its file: `meetings/notes.set.json`
    /// is `meetings.notes.set`; an error variant drops its `.error-...` tail.
    fn method_of(fixture: &str) -> String {
        let stem = fixture.trim_end_matches(".json");
        let stem = stem.split(".error-").next().unwrap_or(stem);
        stem.replace('/', ".")
    }

    #[test]
    fn every_skipped_fixture_has_a_daemon_test_that_calls_its_method() {
        let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivod/tests");
        let mut sources = String::new();
        let mut stack = vec![tests.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)
                .expect("the daemon's tests")
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    sources.push_str(&std::fs::read_to_string(&path).unwrap());
                }
            }
        }
        assert_eq!(method_of("meetings/notes.set.json"), "meetings.notes.set");
        assert_eq!(
            method_of("transfer/chunk.error-rate-limited.json"),
            "transfer.chunk"
        );
        let uncovered: Vec<String> = STATEFUL
            .iter()
            .map(|(fixture, _)| method_of(fixture))
            .filter(|method| !sources.contains(&format!("\"{method}\"")))
            .collect();
        assert!(
            uncovered.is_empty(),
            "the replay skips these as covered by the daemon's tests, but no test under {} calls them: {uncovered:?}",
            tests.display()
        );
    }
}
