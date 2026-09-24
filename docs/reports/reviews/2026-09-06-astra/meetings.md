# Meetings: capture, storage, notes, analysis, exports and migrations

## Verdict

The two-track capture and transcription design is sensible, but persistence, recovery and background-job ownership are not sound enough to trust with meeting records. The most valuable change is to remove whole-row snapshot updates and give each operation a narrow, transactional write over the fields it owns. Read-only SQL checks reproduced lost notes and verified the migration/FTS chain; no files changed, and `just build test lint` was not run because it writes build artifacts.

## Findings

### F1. Capture checkpoints erase notes saved during recording
- **Kind:** bug
- **Where:** `crates/dettivo-meeting/src/worker.rs:404` — `self.archive.updated(&self.row)`; `crates/dettivo-storage/src/meetings.rs:44` — “Rewrites every mutable field”; `crates/dettivod/src/diarization.rs:359` — `s.update_meeting(&row)`.
- **Why:** The capture worker retains the row created at start. Notes update the database independently, but every checkpoint and finalisation writes the old snapshot back, including its empty notes. The SQL reproduction erased both the saved notes and their search hit. Diarization also reads and later rewrites whole rows, exposing notes and analysis to lost updates.
- **Change:** Replace the general update operation with capture-, transcript- and diarization-specific writes. Remove notes and analysis from capture-owned mutable state.
- **Risk:** Fields currently updated incidentally may stop updating. Save notes during recording, cross a checkpoint, stop, and run diarization while editing notes; verify every independent result survives.

### F2. Finalisation can delete the audio and announce success without storing the transcript
- **Kind:** bug
- **Where:** `crates/dettivo-meeting/src/worker_end.rs:190` — `Checkpoint::remove(&ctx.dir)` followed by `apply_artifacts(...)`; line 192 — `if let Err(e) = ctx.archive.updated(&ctx.row)`; line 210 — `.state(&ctx.change(state, State::Transcribing, reason))`.
- **Why:** Cleanup precedes the final database write. If that write fails, the code logs a warning, releases the job and publishes the successful outcome anyway. With audio retention disabled, the audio and checkpoint can already be gone while the database still contains the earlier transcript or transcribing state.
- **Change:** Make successful persistence a prerequisite for cleanup, completion events and subsequent jobs. On storage failure, retain recovery inputs and report the storage error.
- **Risk:** Completion will remain pending when storage is unavailable. Inject a failure into the final archive write and assert that audio remains, no success event appears, and retry can finish.

### F3. Delete the redundant meeting-row copy from metadata
- **Kind:** delete
- **Where:** `crates/dettivo-meeting/src/worker_end.rs:262` — `"meeting": row`; `crates/dettivo-meeting/src/lib.rs:60` — `metadata["meeting"] = serde_json::to_value(row)`; `crates/dettivod/src/handlers/meetings_recovery.rs:37` — the transcript-only branch clears the database and removes only the analysis file.
- **Why:** The metadata sidecar duplicates transcript, segments, notes and potentially analysis from the database. Transcript deletion leaves this copy untouched; a partial meeting’s checkpoint can retain another transcript copy. The delete test cannot detect that leak because `crates/dettivod/tests/meetings_delete.rs:38` writes filenames as placeholder file contents.
- **Change:** Remove the duplicated row block and its rewrite machinery; retain the capture manifest needed for recovery. Clear transcript-bearing checkpoints when deleting a transcript. If the full metadata document must remain supported, regenerate it from the cleared row.
- **Risk:** External consumers may read the metadata document. Check those consumers and test deletion using real populated metadata and checkpoints, asserting that deleted text is absent from every retained artifact.

### F4. Background jobs can restore data after deletion
- **Kind:** bug
- **Where:** `crates/dettivod/src/analysis.rs:291` — `.remove(&row.id)` precedes the result write at line 302; `crates/dettivod/src/handlers/meetings_recovery.rs:31` — `daemon.analysis().cancel(id)`; `crates/dettivod/src/diarization.rs:392` — the speaker pass checks only whether the row still exists.
- **Why:** After analysis removes its running entry, delete cannot cancel it. A transcript-only delete can clear the row, followed by the analysis worker restoring its old summary and analysis. Delete does not cancel diarization at all; because the lighter policies retain the row, the speaker pass can recreate speakers and diarization metadata after they were cleared. Analysis also removes entries by meeting ID without checking job identity, allowing an old cancelled worker to remove a newer job.
- **Change:** Atomically reserve a job identity and retain ownership through commit. Delete must invalidate all jobs for that meeting, and each result must verify its identity and transcript revision before writing.
- **Risk:** Incorrect locking could deadlock completion and deletion. Use barriers to test deletion immediately before commit, simultaneous starts, and cancellation followed by a new job.

### F5. Failed or interrupted finalisation has no supported retry path
- **Kind:** bug
- **Where:** `crates/dettivod/src/history.rs:82` — `store.fail_stale_meeting_jobs(...)`; `crates/dettivod/src/meetings.rs:425` — recovery selects only `Recording` and `Stopping`; `crates/dettivod/src/handlers/meetings_recovery.rs:89` — `row.status != MeetingStatus::Partial`.
- **Why:** A restart during final transcription marks the meeting failed before capture recovery runs. Failed and cancelled finalisations retain audio, but recovery accepts only partial rows, and stop does not restart transcription. The retained inputs become inaccessible through the meeting’s recovery workflow.
- **Change:** Make interrupted capture finalisations recoverable. Provide the same explicit retry operation for failed or cancelled finalisation when validated audio remains.
- **Risk:** Imports and captured meetings have different input layouts. Test restart during transcription, engine failure and cancellation for both, preserving the original meeting ID and notes.

### F6. A microphone cannot return after an unsuccessful reopen
- **Kind:** bug
- **Where:** `crates/dettivo-meeting/src/worker.rs:228` — `None => return Ok(busy)`; line 323 — one `open_microphone(...)` attempt; line 360 — failure only records a gap.
- **Why:** Once the microphone ends, its source is cleared. If the immediate reopen finds no device, nothing subsequently watches for its return or retries opening it. The loop keeps reporting recording while permanently capturing only the system track, or nothing in a microphone-only meeting.
- **Change:** Keep device availability observation independent of the capture handle, or retry reopening at a bounded interval while the microphone is absent. Start a new take with a gap when it returns.
- **Risk:** Repeated attempts must not create empty takes or busy-loop. Unplug the only microphone, wait until reopening fails, reconnect it, and verify later speech is captured.

### F7. Missing capture files are treated as a successful silent meeting
- **Kind:** bug
- **Where:** `crates/dettivo-meeting/src/finalize.rs:65` — `read_sidecar(&sidecar).unwrap_or_default()`; line 68 — `take.samples > 0 && path.is_file()`; line 259 — `Ok(Outcome { ... })`.
- **Why:** An unreadable manifest becomes an empty manifest, and a missing referenced WAV is silently omitted. Finalisation can therefore complete with one side missing or an empty transcript. This also compounds take-close failures, which `crates/dettivo-meeting/src/worker_end.rs:299` merely logs as `"meeting: take not closed"`.
- **Change:** Return errors for missing or unreadable expected inputs and propagate take-finalisation failures. Distinguish an intentionally absent system track and a valid empty capture from damaged capture data.
- **Risk:** Microphone-only meetings must remain valid. Test truncated manifests, missing referenced takes and failed take closure; none should become a completed silent meeting or trigger destructive cleanup.

### F8. Recovery discards the timing information its checkpoint preserved
- **Kind:** bug
- **Where:** `crates/dettivo-meeting/src/recovery.rs:53` — `files.sort()`; lines 58–59 — `start_offset_ms: 0`, `start_offset_ns: 0`; line 86 — the checkpoint is read only after track repair.
- **Why:** When a sidecar is unreadable, recovery enumerates WAVs lexically and assigns every take offset zero. It never uses the checkpoint’s take offsets. Several sequential takes then overlap at the beginning; lexical ordering also puts numbered takes before the original filename. The speaker-track reader can truncate earlier audio when consuming those zero-offset takes.
- **Change:** Recover take identity, order and offsets from the checkpoint and journal, reconciling them with files on disk. Report unknown timing explicitly instead of inventing zero offsets. Calculate recovered duration from the latest take end, including gaps.
- **Risk:** Older or absent checkpoints need a conservative fallback. Corrupt the sidecar of a meeting with three takes and known gaps; verify transcript order, total duration and the complete diarization input.

### F9. The first live windows and final transcript do not share a verified sample clock
- **Kind:** contract
- **Where:** `crates/dettivo-meeting/src/machine.rs:157` — microphone opening precedes system opening; `crates/dettivo-audio/src/takes.rs:108` — `let elapsed = self.started.elapsed()` stamps writer creation; `crates/dettivo-meeting/src/live.rs:129` — `Windower::new(source, settings.clone(), 0)`.
- **Why:** Capture sources open before their writers, and the recorded offsets describe writer creation rather than the first captured samples. Both live lanes nevertheless start at zero, while finalisation applies the stored offsets. Checking that two writers started close together does not establish that their audio samples align.
- **Change:** Carry a first-sample origin through capture into the take manifest and live windower. Use that same origin for finalisation.
- **Risk:** This affects interleaving, gaps and speaker assignment. Delay one source’s startup and inject a shared audible landmark; measure the landmark alignment in live and final output.

### F10. Collapse the completion hooks into one ordered post-processing path
- **Kind:** simplify
- **Where:** `crates/dettivo-meeting/src/worker_end.rs:236` — audio is removed when `!policy.keep_audio`; `crates/dettivod/src/meetings.rs:102` — the completed event invokes the speaker hook; `crates/dettivod/src/analysis.rs:309` — analysis writes its sidecar unconditionally.
- **Why:** Audio retention runs before the completed event starts diarization, so automatic speaker assignment cannot work when audio retention is disabled. With artifacts disabled, successful analysis recreates a meeting directory that finalisation just removed. Two independent completion mechanisms obscure these dependencies.
- **Change:** Give one completion owner responsibility for automatic post-processing and retention. Keep temporary audio until the speaker pass finishes, then apply retention; honor artifact policy when writing analysis and metadata.
- **Risk:** Failed or unavailable post-processing must not retain temporary audio indefinitely. Test automatic analysis and diarization across the audio/artifact policy combinations.

### F11. Filesystem deletion failures leave orphaned files or falsely report success
- **Kind:** bug
- **Where:** `crates/dettivod/src/history.rs:134` — the row is deleted before `self.meetings.remove(id)?`; `crates/dettivo-storage/src/meeting_artifacts.rs:59` — failed `read_dir` returns `Ok(())`; line 62 — `entries.flatten()`.
- **Why:** If full-directory deletion fails, the row is already gone, and retrying the API fails its lookup. For audio-only cleanup, permission and directory-entry errors can be swallowed, after which the daemon clears the audio facts and reports deletion successful despite retained files.
- **Change:** Preserve the addressable row until artifact cleanup succeeds. Treat only absence as successful absence; propagate directory traversal errors and retain accurate facts on partial failure.
- **Risk:** Cleanup can partially succeed before an error. Test permission failures and interrupted traversal, then retry through the API and verify all intended files disappear.

### F12. A meeting row and its speaker table are not updated transactionally
- **Kind:** bug
- **Where:** `crates/dettivo-storage/src/meetings.rs:56` — the meeting UPDATE executes before `replace_speakers` at line 63; `crates/dettivo-storage/src/speakers.rs:42` — `DELETE FROM meeting_speakers` followed by individual INSERTs.
- **Why:** These statements commit separately. A crash or failed insert can leave new segment labels and indexed names alongside an empty or partially replaced speaker table. The SQL reproduction confirmed that a later insert failure does not roll back earlier changes.
- **Change:** Put the meeting update, speaker replacement and associated name changes in one transaction. Only replace speakers when that operation actually changes them.
- **Risk:** Avoid nested transactions in existing callers. Inject failure after speaker deletion and during insertion; the complete previous state should survive.

### F13. Restart leaves analysis and diarization permanently marked running
- **Kind:** bug
- **Where:** `crates/dettivod/src/history.rs:75` — startup settles transcription jobs only; `crates/dettivod/src/analysis.rs:248` — persists `AnalysisStatus::Running`; `crates/dettivod/src/diarization.rs:354` — persists `DiarizationStatus::Running`.
- **Why:** The running maps disappear with the process, but their persisted statuses receive no restart reconciliation. A completed meeting remains “running” without a worker. The UI disables both retry buttons based on those statuses in `qt/qml/Dettivo/app/meetings/AnalysisView.qml:33` and line 43.
- **Change:** On startup, settle orphaned analysis and diarization attempts with an interrupted reason while preserving previous successful results.
- **Risk:** Restart must not erase an earlier analysis or speaker assignment. Kill the daemon during each pass and verify the record remains readable and both retries become available.

### F14. Combined search can hide every meeting hit
- **Kind:** bug
- **Where:** `crates/dettivod/src/handlers/transcripts.rs:307` — dictation hits are appended first; line 331 — meetings follow; line 347 — `items.truncate(limit as usize)`.
- **Why:** With at least `limit` matching dictations, every meeting is discarded regardless of relevance or recency. There is no subsequent page through this handler. A meeting found by meeting search can therefore be invisible in combined history search.
- **Change:** Define and apply one ordering across both kinds before enforcing the limit. Do not assume independently calculated FTS ranks are directly comparable.
- **Risk:** Search ordering will change. Test more than one page’s worth of dictation matches plus an exact meeting-title match and ensure the meeting remains reachable.

### F15. Status polling unnecessarily loads the entire meeting archive
- **Kind:** simplify
- **Where:** `crates/dettivo-storage/src/meetings.rs:147` — `SELECT {COLUMNS} FROM meetings ORDER BY created_at, seq`; line 154 — status filtering happens in Rust; `crates/dettivod/src/handlers/meetings.rs:432` — every status response calls `recoverable(daemon)?`.
- **Why:** Each poll deserializes every meeting’s transcript, notes and analysis just to identify partial meetings. List and search also materialize the full wide row and attach speakers individually before discarding most fields. All of this runs under the shared store lock.
- **Change:** Filter statuses in SQL and introduce small read projections for status, recovery lists, search hits and list rows. Load the full meeting only for detail and export.
- **Risk:** Summary projections must preserve wire fields and cursor ordering. Test a large archive containing long transcripts and maximum-size notes; inspect rows read and response latency.

### F16. Invalid stored JSON silently becomes missing data
- **Kind:** bug
- **Where:** `crates/dettivo-storage/src/meeting_row.rs:342` — `serde_json::from_str(&s).ok()` followed by `unwrap_or_default()`; lines 348 and 353 apply the same suppression to diarization and analysis.
- **Why:** Malformed or incompatible stored data reads as empty segments or absent analysis without an error. A subsequent whole-row update can permanently replace the original stored content with that empty interpretation. Successful reads and exports conceal the problem.
- **Change:** Distinguish SQL NULL from invalid JSON. Return a corruption/compatibility error identifying the meeting and column, and prevent unrelated updates from rewriting unreadable payloads.
- **Risk:** Previously hidden bad records will become visible errors. Test malformed JSON and older supported payloads separately, preserving original bytes on failure.

### F17. An empty notes override restores the saved notes
- **Kind:** contract
- **Where:** `crates/dettivo-storage/src/meeting_export.rs:129` — `.filter(|n| !n.trim().is_empty())` before falling back to saved notes; `qt/host/app/history_actions.cpp:179` — `if (!notesOverride.isEmpty())`.
- **Why:** Clearing the live editor and exporting can include the old saved notes. Both the client and renderer collapse “explicitly empty override” into “no override,” violating the documented precedence.
- **Change:** Preserve the distinction between absent and explicitly empty overrides. Choose the override first, then decide whether the resulting Notes section is empty.
- **Risk:** Callers currently using an empty string to mean “use saved notes” must pass absence instead. Add Markdown and JSON cases for absent, empty, whitespace-only and nonempty overrides.

### F18. Valid speaker names can break subtitle exports
- **Kind:** bug
- **Where:** `crates/dettivod/src/handlers/meetings_speakers.rs:48` — validation checks character count; `crates/dettivo-storage/src/meeting_export.rs:165` — `"{}\n{} --> {}\n{}: {}\n\n"` interpolates the label unchanged.
- **Why:** A name such as `Ada\n\nBob` passes validation and inserts a subtitle block separator into the cue payload. The same unrestricted interpolation exists in VTT. Ordinary golden fixtures cannot establish that arbitrary valid meeting content produces valid subtitles.
- **Change:** Reject control characters in speaker names and normalize subtitle payload line breaks; escape format-specific markup where necessary. Keep stored transcript text unchanged.
- **Risk:** Existing names containing control characters need safe rendering. Validate generated subtitles with a parser using multiline text, blank lines and markup characters, alongside the existing goldens.

### F19. The decision record names migrations that do not exist
- **Kind:** record
- **Where:** `docs/adr/0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md:21` — “migration `0005-speakers`”; `docs/adr/0036-meeting-notes-analysis-search-export-and-the-delete-policy.md:17` — “Migration `0005-notes-analysis`”; `crates/dettivo-storage/src/migrate.rs:49` — `name: "0005-timings"`.
- **Why:** The executable chain assigns speakers to migration 6 and notes/analysis to migration 7. ADR 0036 also describes six indexed columns, while the actual index has seven including speaker names. These discrepancies undermine the record used to assess upgrades.
- **Change:** Add explicit corrections to the records and update the corresponding history documentation. Preserve the existing migration numbering.
- **Risk:** Documentation-only. Check every migration reference against the registry and describe the complete FTS column set.

## Keep

- The shared offline transcription pipeline and explicit take offsets in `crates/dettivo-meeting/src/finalize.rs`. Fix input validation and clock provenance around it.
- The conservative coverage/share speaker assignment in `crates/dettivo-meeting/src/diarize.rs`. Ambiguous spans correctly remain unlabelled.
- The narrow analysis setters in `crates/dettivo-storage/src/meeting_notes.rs`. They preserve user notes and retain previous analysis after a failed regeneration.
- The seven-migration chain and transactional FTS rebuilds in `crates/dettivo-storage/src/migrate.rs` and `crates/dettivo-storage/migrations/0007-notes-analysis.sql`. In-memory checks passed for all four upgrade fixtures and a populated version-6 speaker record.
- Parameterized FTS queries and column-attributed snippets in `crates/dettivo-storage/src/search.rs` and `crates/dettivo-storage/src/timeline.rs`.
- The five deterministic export goldens in `crates/dettivo-storage/tests/meeting_export.rs`. Extend their edge coverage; a rendering framework would add little.
- File-size discipline. All 46 scoped Rust files inspected are below 500 lines.

## Questions for the owner

- What audio-loss bound is required for power failure? The current checkpoint path uses flush and rename without explicit durable file/directory synchronization; the record promises survival beyond a daemon crash.
- Should a meeting’s `analyze=false` choice survive recovery? It currently lives only in the in-memory override map in `crates/dettivod/src/analysis.rs:92`, so recovery falls back to the global automatic-analysis setting.