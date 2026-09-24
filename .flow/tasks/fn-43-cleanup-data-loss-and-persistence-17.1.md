---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18]
---
# fn-43-cleanup-data-loss-and-persistence-17.1 Cleanup: data loss and persistence, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-43 cleanup: data loss and persistence (17 findings)

Every finding was verified against the code and a test before it was touched; every one held and was fixed. Each fix carries a regression test that failed before the fix and passes after it (the two exceptions are noted in the table: a registry that did not exist before and a QA helper that is the fix). No finding was rejected.

| Finding | Outcome | Commit and evidence |
|---|---|---|
| R1 daemon/F1, R4 meetings/F1: checkpoints erase notes saved during recording | fixed | 95302bb9 `update_meeting` rewrites only the 28 row-owned columns; summary, notes and analysis stay with their setters. Test `dettivo-storage::a_whole_row_update_keeps_the_notes_and_the_analysis_saved_meanwhile` (failed before: notes gone; passes after) |
| R2 agent-surfaces/F1: failed policy lookup becomes delete "all" | fixed | 4575b7b2 `meetings.delete` resolves an omitted `artifact_policy` from `[meetings] delete_artifact_policy` (additive delta registered in `docs/api/linux-deltas.md`, fixture `meetings/delete.json`); the CLI sends no policy and the `config.get` fallback is deleted. Tests `dettivod::an_omitted_policy_is_the_configured_one` (INVALID_PARAMS before), `dettivo-cli::a_delete_without_a_policy_leaves_the_policy_to_the_daemon` |
| R3 dictation/F11: completion in a global slot | fixed | c45c4746 a per-take `Completion` (condvar) created before the source opens; stop and cancel wait on it; a panicked worker completes it failed. Test `dettivo-session::every_stopper_of_a_take_gets_that_takes_result` (before: `[Err(NoSession), Err(NoSession)]`) |
| R5 meetings/F2: finalisation cleans up before storing | fixed | 3a6950c8 the settled row is stored first; a refused store keeps the takes and the checkpoint, ends the meeting failed with the storage error, publishes no completion; `settled_facts` keeps the stored row and the directory in agreement. Test `dettivo-meeting::a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed` (before: takes removed) |
| R6 meetings/F4: background jobs restore data after a delete | fixed | ab99c777 one `MeetingJobs` registry per meeting (`crates/dettivod/src/meeting_jobs.rs`): results commit by job identity under the lock a cancel or delete takes; `meetings.delete` invalidates the analysis and the speaker pass; a late job cannot remove a newer entry. Barrier tests `an_old_job_neither_removes_nor_commits_over_a_newer_one`, `a_cancel_keeps_the_entry_and_an_invalidation_drops_the_result`, `an_invalidation_during_a_commit_waits_for_the_write` (the registry is new; the old code's `remove(&row.id)` before the write and the missing diarization cancel are the evidence) |
| R7 meetings/F11: deletion failures orphan files or report success | fixed | 8a546278 directory before row in `History::delete_meeting`; `remove_matching` treats only a missing directory as nothing and propagates read errors. Tests `dettivo-storage::an_unreadable_directory_is_an_error_and_a_missing_one_is_fine` (before: `Ok`), `dettivod::a_directory_that_cannot_be_cleaned_keeps_the_facts_and_the_row_until_a_retry` (before: NOT_FOUND on retry); both skip as root |
| R8 meetings/F12: row and speakers not transactional | fixed | 95302bb9 insert, update and `clear_meeting_transcript` run the row and the speakers in one `unchecked_transaction`; speakers replaced only when they differ. Test `dettivo-storage::the_row_and_its_speakers_change_together_or_not_at_all` (before: title "After", speakers half replaced) |
| R9 meetings/F16: invalid JSON reads as missing data | fixed | 95302bb9 `json_column` distinguishes NULL from unparseable text and errors naming the meeting and column. Test `dettivo-storage::unreadable_json_is_an_error_and_null_is_absent` (older payload without the newer keys still reads) |
| R10 qa-rig/F4: scenario config drops the hotkeys baseline | fixed | 2f812b0d `profile::with_baseline` appends `[hotkeys] backend = "none"` unless the scenario names a `[hotkeys]` table; `DaemonHandle::spawn` writes it; the 14 scenario copies deleted. Test `dettivo-qa::the_baseline_keeps_the_hotkey_backend_off_unless_the_scenario_names_one` (the helper is the fix) |
| R11 qa-packs/F9: normalisation erases types and array structure | fixed | c495f68d `replay_shape.rs`: typed leaves, merged row shape per list, `shapes_agree` tolerating only empty lists and nulls; shape-compared methods use it. Test `dettivo-qa::typed_shapes_catch_a_wrong_type_and_a_malformed_element`; `dettivo-qa contract` and `--strict` stay green |
| R12 qa-packs/F14: checksum lost during cleanup | fixed | ecd7e2b6 `run_pipeline` hashes while the runner and profile exist; a real candidate without a hash is an error. Test `dettivo-qa::a_real_candidate_needs_its_checksum_while_the_file_exists` |
| R13 qt-hosts/F1: app close restores an old acknowledgement | fixed | f32393ee `AppState::save` reads the daemon's tables from the file at save time; `otherTables` deleted. Test `qt::stateFileRoundTrips` (before: `meeting_disclosure = false` written back) |
| R14 qt-hosts/F3: failed export destroys the destination | fixed | 44cd33df one `QSaveFile` per transfer, committed at eof. Test `qt::actionsRerunDeleteAndExportThroughTheLink` (first chunk and second chunk failures keep the sentinel) |
| R15 qt-hosts/F10: old OSD completion overwrites a newer take | fixed | 2fd36db8 a take counter guards the dwell timer; recording, failure and hide bump it. Test `qt::anOldTakesCompletionNeverOverwritesANewerTake` |
| R16 qt-hosts/F13: model actions erase their own failure | fixed | e782393d refresh no longer clears `lastRefusal`; the next attempt's answer replaces it. Test in `qt::modelsTableOrdersRowsAndFollowsADownload` |
| R17 ops-and-record/F9: conversion removes the old model first | fixed | 7c506957 staging directory for convert, quantise, checksum and verify; publish then manifest. Test `scripts/models/tests/convert-polish-finetune-staging.sh` (before: "the model changed"), wired into `polish-convert-check` in Makefile and justfile |
| R18 gate, ADR, replay, summary | done | 5f09dd77 ADR 0045 (indexed; amends 0024, 0027, 0030, 0032, 0035, 0036, 0043); `qa/evidence-map.toml` routes every R-ID to its test; gate and contract replays below |

### ADR

`docs/adr/0045-a-write-owns-its-fields-and-nothing-announces-what-it-did-not-store.md`, indexed in `docs/adr/README.md`. Earlier records are amended, none superseded, so their status lines stand.

### Verification

- `flock /tmp/dtv-gate.lock make build test lint` at 24f624d7 (after the rebase onto origin/main a47c0c5): exit 0 at 17:03 UTC; two lint-only follow-ups after the first two runs (84a390a1 clippy `nonminimal_bool` and `useless_format`, 24f624d7 a rustdoc intra-doc link in the CLI help), the build and every test stage passed on all three runs.
- `cargo run -q -p dettivo-qa -- contract` and `-- contract --strict`: green (socket and REST replays, 0 failed).
- Per-crate before/after runs for every regression test as noted in the table; Qt tests through `build/qt` (`dettivo-osd-host-test`, `dettivo-app-host-test`, `dettivo-app-history-test`, `dettivo-settings-test`).
- `~/.local/share/dettivo/models` byte-identical before and after (no drive was needed; nothing under `scripts/qa/xvfb-session.sh` was run because no fix touches a drive).

### Deliberately left out

- meetings/F3 (the metadata row copy), F5 (retry path for failed finalisations) and F10 (completion hooks) belong to other specs; R5 notes in the ADR that a storage-failed meeting's retry path is F5's.
- The app's `MeetingsActions::deletePolicy()` still names a policy from its settings model (falls back to "all" when the config model has none); the finding was about the CLI's failed lookup, and the app reads an in-memory config it already loaded rather than issuing a request per delete.
- The daemon-side `contract.rs` test keeps its own value-levelling normaliser; the finding named the replay in `dettivo-qa`.
- origin/main 5b5f67b left 132 absolute links in the checked-in Astra reports that failed `scripts/check-docs.sh`; the conductor's a47c0c5 fixes them, and the branch's own copy of that fix was dropped at the rebase.
## Evidence
- Commits: 246d9b4f62d3183526247d6ea9917b1525f80f15, 47bfa9cc6e2e8537cd0b46aadc76b479a8d2ebf0, 42f3c28d5c43ac71fd1bbfdb581248272b93ffb7, e77499fab7195a7ed8f871254adb463725a83880, 0cf634ee1ed73de5d6e87d9614f03f43a5cced69, 80801f99b6bf9f02941653b1996b16874b7f97fa, 912b65f8caf468039baa7b6cae5981859ef6f42e, 897bc08c51577f2107e750f81945e8d60849601e, 71ce35beca09cf8a40cb2f40e077207df3c7c97f, 3cf4424287bbd7b62c5d5f6aba620df7bc79295c, fe792218a9248ef7a1f287a5e3b374451d53f740, 7dba4384f7a4bef9dcc9b99afc2566f7b61f8c71, ae9c56b4bc53f779accbe790e56e94d9fc51f627, 1941e98568aa4ff1f3c25791e5b5c867d6752a0c, 71ceae90fcccffd954b56a15e891646f80bdc535, e4fa1b1b457732c19685376dd31f064f2cb05a06, b1e65f1125a8ad0208d359798afdd382b1f5b237
- Tests: flock /tmp/dtv-gate.lock make build test lint, cargo run -q -p dettivo-qa -- contract, cargo run -q -p dettivo-qa -- contract --strict, cargo test -p dettivo-storage, cargo test -p dettivo-meeting, cargo test -p dettivo-session, cargo test -p dettivod --bin dettivod meeting_jobs, cargo test -p dettivod --test meetings_delete --test meetings_notes --test meetings_diarize --test meetings --test contract, cargo test -p dettivo-cli --test meetings, cargo test -p dettivo-proto, cargo test -p dettivo-qa --lib, scripts/models/tests/convert-polish-finetune-staging.sh, build/qt/host/osd/dettivo-osd-host-test, build/qt/host/app/dettivo-app-host-test, build/qt/host/app/dettivo-app-history-test, build/qt/host/app/dettivo-settings-test
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
