# Cleanup: data loss and persistence (17 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

Every write owns exactly the fields its operation changes, nothing announces success it did not store, and a delete removes exactly what its policy names. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0045 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F1** (bug): Meeting checkpoints overwrite notes saved during recording
  - Where: `crates/dettivod/src/history.rs:345`: `self.with_store(|store| store.update_meeting(row))`; `crates/dettivo-storage/src/meetings.rs:44`: “Rewrites every mutable field of a meeting”; `crates/dettivo-meeting/src/worker.rs:404`: `self.archive.updated(&self.row)`.
  - Why: The recording worker retains its original row, including empty notes. Saving notes updates the database independently. The next checkpoint writes the worker’s stale row across all mutable columns, erasing the saved notes. Finalization and import completion use the same broad update mechanism.
  - Change: Replace whole-row updates with updates restricted to fields owned by capture, transcription, notes or analysis. Preserve independently edited fields at the database boundary.
  - Risk: Metadata and database state could diverge during the change. Save notes during recording and verify them after multiple checkpoints, stop, restart and import completion.

### agent-surfaces

- **agent-surfaces/F1** (bug): A failed policy lookup turns meeting deletion into “delete everything”
  - Where: `crates/dettivo-cli/src/meetings.rs:362` — `.ok()` followed by `.unwrap_or_else(|| "all".to_string())`.
  - Why: A timeout, authorization error or malformed configuration response becomes the most destructive artifact policy. If the subsequent delete succeeds, someone who configured `transcript_only` can lose the meeting’s audio, notes and row.
  - Change: Propagate the lookup failure immediately. Move resolution of an omitted policy into the daemon’s delete operation, removing this client-side policy decision and its extra request.
  - Risk: Preserve explicit policy arguments and record any additive contract change. Test a failed configuration lookup followed by an otherwise available daemon, asserting that no delete occurs.

### dictation

- **dictation/F11** (bug): Session completion belongs to a global slot that another take can overwrite
  - Where: `crates/dettivo-session/src/machine.rs:163` — `*self.outcome.lock()... = None`; `crates/dettivo-session/src/machine.rs:239` — `(a.stop.clone(), a.worker.take())`; `crates/dettivo-session/src/machine.rs:245` — reading `self.outcome`.
  - Why: Stop joins one worker but retrieves its result from a shared, resettable slot. A new take can clear that result before the old stopper reads it. Concurrent stoppers also compete for the single join handle; a stop during source opening sees no worker and can return `NoSession` despite an active reservation.
  - Change: Create a completion object for each take before opening its source. Let all stop/cancel callers wait on that take’s result, and publish completion consistently with active-state retirement.
  - Risk: Start failure and shutdown need to complete the same object. Use barriers to test stop during source opening, two simultaneous stoppers and a new start immediately after completion.

### meetings

- **meetings/F1** (bug): Capture checkpoints erase notes saved during recording
  - Where: `crates/dettivo-meeting/src/worker.rs:404` — `self.archive.updated(&self.row)`; `crates/dettivo-storage/src/meetings.rs:44` — “Rewrites every mutable field”; `crates/dettivod/src/diarization.rs:359` — `s.update_meeting(&row)`.
  - Why: The capture worker retains the row created at start. Notes update the database independently, but every checkpoint and finalisation writes the old snapshot back, including its empty notes. The SQL reproduction erased both the saved notes and their search hit. Diarization also reads and later rewrites whole rows, exposing notes and analysis to lost updates.
  - Change: Replace the general update operation with capture-, transcript- and diarization-specific writes. Remove notes and analysis from capture-owned mutable state.
  - Risk: Fields currently updated incidentally may stop updating. Save notes during recording, cross a checkpoint, stop, and run diarization while editing notes; verify every independent result survives.

- **meetings/F2** (bug): Finalisation can delete the audio and announce success without storing the transcript
  - Where: `crates/dettivo-meeting/src/worker_end.rs:190` — `Checkpoint::remove(&ctx.dir)` followed by `apply_artifacts(...)`; line 192 — `if let Err(e) = ctx.archive.updated(&ctx.row)`; line 210 — `.state(&ctx.change(state, State::Transcribing, reason))`.
  - Why: Cleanup precedes the final database write. If that write fails, the code logs a warning, releases the job and publishes the successful outcome anyway. With audio retention disabled, the audio and checkpoint can already be gone while the database still contains the earlier transcript or transcribing state.
  - Change: Make successful persistence a prerequisite for cleanup, completion events and subsequent jobs. On storage failure, retain recovery inputs and report the storage error.
  - Risk: Completion will remain pending when storage is unavailable. Inject a failure into the final archive write and assert that audio remains, no success event appears, and retry can finish.

- **meetings/F4** (bug): Background jobs can restore data after deletion
  - Where: `crates/dettivod/src/analysis.rs:291` — `.remove(&row.id)` precedes the result write at line 302; `crates/dettivod/src/handlers/meetings_recovery.rs:31` — `daemon.analysis().cancel(id)`; `crates/dettivod/src/diarization.rs:392` — the speaker pass checks only whether the row still exists.
  - Why: After analysis removes its running entry, delete cannot cancel it. A transcript-only delete can clear the row, followed by the analysis worker restoring its old summary and analysis. Delete does not cancel diarization at all; because the lighter policies retain the row, the speaker pass can recreate speakers and diarization metadata after they were cleared. Analysis also removes entries by meeting ID without checking job identity, allowing an old cancelled worker to remove a newer job.
  - Change: Atomically reserve a job identity and retain ownership through commit. Delete must invalidate all jobs for that meeting, and each result must verify its identity and transcript revision before writing.
  - Risk: Incorrect locking could deadlock completion and deletion. Use barriers to test deletion immediately before commit, simultaneous starts, and cancellation followed by a new job.

- **meetings/F11** (bug): Filesystem deletion failures leave orphaned files or falsely report success
  - Where: `crates/dettivod/src/history.rs:134` — the row is deleted before `self.meetings.remove(id)?`; `crates/dettivo-storage/src/meeting_artifacts.rs:59` — failed `read_dir` returns `Ok(())`; line 62 — `entries.flatten()`.
  - Why: If full-directory deletion fails, the row is already gone, and retrying the API fails its lookup. For audio-only cleanup, permission and directory-entry errors can be swallowed, after which the daemon clears the audio facts and reports deletion successful despite retained files.
  - Change: Preserve the addressable row until artifact cleanup succeeds. Treat only absence as successful absence; propagate directory traversal errors and retain accurate facts on partial failure.
  - Risk: Cleanup can partially succeed before an error. Test permission failures and interrupted traversal, then retry through the API and verify all intended files disappear.

- **meetings/F12** (bug): A meeting row and its speaker table are not updated transactionally
  - Where: `crates/dettivo-storage/src/meetings.rs:56` — the meeting UPDATE executes before `replace_speakers` at line 63; `crates/dettivo-storage/src/speakers.rs:42` — `DELETE FROM meeting_speakers` followed by individual INSERTs.
  - Why: These statements commit separately. A crash or failed insert can leave new segment labels and indexed names alongside an empty or partially replaced speaker table. The SQL reproduction confirmed that a later insert failure does not roll back earlier changes.
  - Change: Put the meeting update, speaker replacement and associated name changes in one transaction. Only replace speakers when that operation actually changes them.
  - Risk: Avoid nested transactions in existing callers. Inject failure after speaker deletion and during insertion; the complete previous state should survive.

- **meetings/F16** (bug): Invalid stored JSON silently becomes missing data
  - Where: `crates/dettivo-storage/src/meeting_row.rs:342` — `serde_json::from_str(&s).ok()` followed by `unwrap_or_default()`; lines 348 and 353 apply the same suppression to diarization and analysis.
  - Why: Malformed or incompatible stored data reads as empty segments or absent analysis without an error. A subsequent whole-row update can permanently replace the original stored content with that empty interpretation. Successful reads and exports conceal the problem.
  - Change: Distinguish SQL NULL from invalid JSON. Return a corruption/compatibility error identifying the meeting and column, and prevent unrelated updates from rewriting unreadable payloads.
  - Risk: Previously hidden bad records will become visible errors. Test malformed JSON and older supported payloads separately, preserving original bytes on failure.

### qa-rig

- **qa-rig/F4** (bug): Scenario configuration overwrites the profile’s disabled-hotkeys baseline
  - Where: `crates/dettivo-qa/src/profile.rs:85`: `"[hotkeys]\nbackend = \"none\"\n"`; `crates/dettivo-qa/src/scenarios/daemon.rs:59`: `std::fs::write(&config_file, config)`; `crates/dettivo-core/src/config/schema.rs:125`: `backend: "auto".into()`.
  - Why: The generic launcher replaces the seeded configuration. Helpers such as `daemon_config` omit hotkeys, restoring the product’s automatic backend and allowing desktop shortcut registration or portal interaction during ordinary GUI scenarios.
  - Change: Merge scenario settings into a profile baseline through one configuration helper. Require explicit selection of a real hotkey backend.
  - Risk: Actual hotkey scenarios need their overrides. Check the final parsed configuration and backend status for ordinary and hotkey-specific launches.

### qa-packs

- **qa-packs/F9** (contract): Contract normalization erases types and entire array element contracts
  - Where: `crates/dettivo-qa/src/replay.rs:246` — `Value::Array(_) => Value::String("list".into())`; `crates/dettivo-qa/src/replay.rs:250` — `_ => Value::String("leaf".into())`.
  - Why: A number, boolean, string and null become identical. Arrays with malformed elements compare equal to correctly structured arrays. This affects engine, device, selection and history results. Normalizing machine-specific values should not allow the transport to violate field types or row structure.
  - Change: Validate typed response shapes before normalizing values. Preserve array element structure and declared nullability; tolerate only the particular fields whose values vary.
  - Risk: Legitimate optional fields need accurate schemas. Check wrong scalar types, malformed array entries and permitted empty arrays or null values.

- **qa-packs/F14** (bug): Catalogue-model evaluation loses its checksum during cleanup
  - Where: `crates/dettivo-qa/src/polish_eval/mod.rs:160` — `model_file: runner.model_file.clone()`; `crates/dettivo-qa/src/polish_eval/mod.rs:305` — `model_sha256: model_file`; `crates/dettivo-qa/src/profile.rs:222` — `models.remove()`.
  - Why: `run_pipeline` returns the model path and drops its runner and profile. The caller then hashes that deleted path. Both checked-in Qwen3-4B evaluation reports contain temporary model paths and omit `model_sha256`; the external sideload reports retain checksums because their files survive cleanup.
  - Change: Hash the loaded model while the profile exists and return the checksum with the pipeline result. Treat an unavailable checksum for a real candidate as an evidence failure.
  - Risk: Large model hashing adds read time. Verify catalogue, direct-path and sideload candidates retain correct hashes after cleanup.

### qt-hosts

- **qt-hosts/F1** (bug): Closing the app can erase a meeting acknowledgement recorded while it was open
  - Where: `qt/host/app/app_state.cpp:92`, `s.otherTables = QString::fromStdString(rest.str()).trimmed();`; `qt/host/app/app_state.cpp:110`, `out += QStringLiteral("\n") + otherTables + QStringLiteral("\n");`.
  - Why: The app preserves daemon-owned tables from its startup snapshot, then writes that snapshot back on exit. Meanwhile, `crates/dettivod/src/meetings.rs:410` independently loads and updates the same file to record disclosure acknowledgement. Opening the app, acknowledging disclosure and closing the app can therefore restore the old acknowledgement. Atomic replacement prevents partial files; it does not prevent this lost update.
  - Change: Give the file one writer, or move app-owned state into a separate file. Delete the app’s preservation and rewriting of daemon-owned tables.
  - Risk: Existing window geometry and first-run completion need migration. Test acknowledgement and daemon restart while the app remains open, then close and reopen it.

- **qt-hosts/F3** (bug): A failed export can destroy an existing destination file
  - Where: `qt/host/app/history_actions.cpp:271`, `file.open(QIODevice::WriteOnly | QIODevice::Truncate)`; `qt/host/app/history_actions.cpp:278`, `m_link->call(QStringLiteral("transfer.pull"), params,`.
  - Why: Export truncates the chosen destination before retrieving the first chunk. A daemon disconnect or transfer failure leaves an empty or partial file where the user’s previous export existed.
  - Change: Keep one `QSaveFile` for the transfer and commit only after successful completion. Delete the repeated truncate/open/append sequence.
  - Risk: Preserve overwrite confirmation and cancellation behavior. Export over a sentinel file, fail the first and a later chunk, and verify the sentinel survives unchanged.

- **qt-hosts/F10** (bug): An old OSD completion can overwrite a newer take
  - Where: `qt/host/osd/osd_model.cpp:264`, the delayed callback captures `payload, job`; line 265 checks only `m_state == QStringLiteral("transcribing")`.
  - Why: Take A completes during its minimum dwell and schedules a timer. Take B starts and reaches Transcribing before that timer fires. The condition succeeds, so A’s completion replaces B’s state and text. Capturing A’s job ID does not help because it is used only for logging.
  - Change: Cancel the pending completion on a new session or guard it with a session generation. A matching state name is insufficient.
  - Risk: Test two short takes inside the dwell interval, plus cancellation and daemon disconnection while a completion is pending.

- **qt-hosts/F13** (bug): Model actions immediately erase their own failure message
  - Where: `qt/host/app/models_table.cpp:305–308`, `emit refused(m_lastRefusal);` followed by `refresh();`; `qt/host/app/models_table.cpp:109–110`, `m_lastRefusal.clear();` followed by `emit refused(QString());`.
  - Why: Every refused download, deletion or selection emits its error and then clears it synchronously through refresh. QML can receive the failure and empty string before rendering a frame, leaving the user with no explanation.
  - Change: Keep the action error until dismissal, another explicit attempt or successful resolution. Background refresh must not clear it.
  - Risk: Avoid retaining an obsolete error after success. Assert the error remains observable after the refresh replies finish.

### ops-and-record

- **ops-and-record/F9** (bug): Fine-tune conversion destroys the previous model before succeeding
  - Where: `scripts/models/convert-polish-finetune.sh:218` removes `"$old" "$old.sha256"`; conversion starts at line 225, and optional verification follows manifest publication at line 254.
  - Why: Reusing an experiment ID deletes its working model before conversion, quantization, or verification succeeds. A failed replacement leaves the existing manifest pointing at missing or incomplete content.
  - Change: Build and verify in a staging directory, then publish the completed model and manifest. Preserve the previous experiment until replacement succeeds.
  - Risk: Publication order matters when the daemon loads concurrently. Inject failures at conversion, quantization, and verification and confirm the previous model remains usable.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F1, Meeting checkpoints overwrite notes saved during recording: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** agent-surfaces/F1, A failed policy lookup turns meeting deletion into “delete everything”: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** dictation/F11, Session completion belongs to a global slot that another take can overwrite: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** meetings/F1, Capture checkpoints erase notes saved during recording: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** meetings/F2, Finalisation can delete the audio and announce success without storing the transcript: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** meetings/F4, Background jobs can restore data after deletion: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** meetings/F11, Filesystem deletion failures leave orphaned files or falsely report success: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** meetings/F12, A meeting row and its speaker table are not updated transactionally: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** meetings/F16, Invalid stored JSON silently becomes missing data: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** qa-rig/F4, Scenario configuration overwrites the profile’s disabled-hotkeys baseline: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** qa-packs/F9, Contract normalization erases types and entire array element contracts: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** qa-packs/F14, Catalogue-model evaluation loses its checksum during cleanup: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** qt-hosts/F1, Closing the app can erase a meeting acknowledgement recorded while it was open: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** qt-hosts/F3, A failed export can destroy an existing destination file: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** qt-hosts/F10, An old OSD completion can overwrite a newer take: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** qt-hosts/F13, Model actions immediately erase their own failure message: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** ops-and-record/F9, Fine-tune conversion destroys the previous model before succeeding: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0045 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
