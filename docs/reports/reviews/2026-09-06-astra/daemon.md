# The daemon, its contract and configuration

## Verdict

The separation between daemon, typed protocol, configuration schema and supervised engines is sound. The implementation is not ready to call complete: ordinary operations can overwrite saved data, concurrent requests can violate session ownership, and passing fixture checks can conceal broken behavior. The most valuable change is to give each operation explicit ownership through its durable commit, replacing stale whole-row writes and separate check-then-act steps.

## Findings

### F1. Meeting checkpoints overwrite notes saved during recording

- **Kind:** bug
- **Where:** `crates/dettivod/src/history.rs:345`: `self.with_store(|store| store.update_meeting(row))`; `crates/dettivo-storage/src/meetings.rs:44`: “Rewrites every mutable field of a meeting”; `crates/dettivo-meeting/src/worker.rs:404`: `self.archive.updated(&self.row)`.
- **Why:** The recording worker retains its original row, including empty notes. Saving notes updates the database independently. The next checkpoint writes the worker’s stale row across all mutable columns, erasing the saved notes. Finalization and import completion use the same broad update mechanism.
- **Change:** Replace whole-row updates with updates restricted to fields owned by capture, transcription, notes or analysis. Preserve independently edited fields at the database boundary.
- **Risk:** Metadata and database state could diverge during the change. Save notes during recording and verify them after multiple checkpoints, stop, restart and import completion.

### F2. A second daemon modifies the first daemon’s work before refusing to start

- **Kind:** bug
- **Where:** `crates/dettivod/src/main.rs:112`: `daemon.meetings().recover_stale(daemon.history())`, before line 123’s `listener::acquire(&daemon).await`; `crates/dettivod/src/history.rs:77`: `store.fail_stale_jobs("the daemon restarted before the job finished")?`.
- **Why:** Construction opens and mutates the database before checking whether another instance owns the socket. Recovery assumes all unfinished jobs and recordings belong to a dead process. A duplicate launch can therefore mark active imports failed and recover a meeting that the first daemon is still recording.
- **Change:** Acquire exclusive ownership of the data store before migrations or recovery. Begin accepting requests only after recovery completes. Socket selection alone should not permit two daemons to recover the same database.
- **Risk:** Preserve socket activation and genuine crash recovery. Launch a second daemon during recording and importing; verify that it exits without changing rows or audio files.

### F3. Invalid configuration silently removes the extra authentication requirement

- **Kind:** contract
- **Where:** `crates/dettivo-core/src/config/mod.rs:74`: `Err(err) => (Config::default(), BTreeMap::new(), Some(err))`; `docs/config.md:33`: “the daemon still starts, on defaults”.
- **Why:** Reload installs this fallback configuration. The default IPC mode is `peer`, so a malformed edit while using `peer_token` removes token enforcement for same-user clients. Other explicit settings, including retention settings, also revert. This follows the documented decision; the decision itself is unsafe.
- **Change:** Retain the last valid effective configuration on reload failure and report the rejected file separately. For an invalid existing file at startup, refuse normal startup or provide an explicitly restricted repair mode. Keep defaults for a missing file.
- **Risk:** This changes documented recovery behavior. Test malformed edits while token authentication is enabled, checking both authentication and retained settings.

### F4. The configuration lock does not cover the reads that determine edits

- **Kind:** bug
- **Where:** `crates/dettivod/src/handlers/polish.rs:128`: `let mut rules = daemon.config().config.polish.rules.clone();`; `crates/dettivod/src/handlers/config.rs:86`: `let _guard = daemon.config_edit_lock();`; `crates/dettivod/src/watcher.rs:29`: `let loaded = daemon.reload();`.
- **Why:** Rule and trusted-endpoint handlers derive replacement collections before acquiring the edit lock. Concurrent additions can overwrite each other, and an update can restore a deleted entry. The watcher also reloads outside this lock, allowing an older snapshot to be installed after a newer edit.
- **Change:** Serialize reading, deriving, validating, writing and installing a configuration snapshot. Derive collection changes from the file read inside that transaction. Route watcher installation through the same synchronization.
- **Risk:** Avoid recursive locking when an edit reloads. Use barriers to test simultaneous additions, update/delete races and watcher overlap.

### F5. Analysis cancellation and completion race with deletion

- **Kind:** bug
- **Where:** `crates/dettivod/src/analysis.rs:291`: `.remove(&row.id)`, before line 302’s `s.set_meeting_analysis(...)`; `crates/dettivod/src/handlers/meetings_recovery.rs:31`: `daemon.analysis().cancel(id)`.
- **Why:** Completion removes the running job before committing its result. Deletion in that interval sees nothing to cancel, clears the transcript and analysis, then the old worker restores the analysis. Admission has a related gap: `start` checks for an existing job separately from inserting the new one, permitting duplicate analyses whose cleanup can remove each other’s registration.
- **Change:** Use one per-meeting lifecycle guard for admission, cancellation and final publication. Check job identity when completing; keep ownership until database and file publication finish.
- **Risk:** Preserve cancellation responsiveness without holding a lock during inference. Test simultaneous starts, cancellation followed by restart, and deletion paused immediately before result commit.

### F6. Dictation and meeting starts can both pass their exclusion checks

- **Kind:** bug
- **Where:** `crates/dettivod/src/actions.rs:78`: `if daemon.meetings().snapshot().is_some()`; `crates/dettivod/src/handlers/meetings.rs:179`: `if daemon.dictation().snapshot().is_some()`.
- **Why:** Each route checks the other session machine, then starts its own independently. Two concurrent connections can both observe idle state and proceed. The individual session locks prevent duplicate sessions of one kind, but do not enforce the advertised exclusion between kinds.
- **Change:** Introduce one daemon-owned capture reservation shared by dictation, meetings and hotkey starts. Acquire it before setup and release it on every failure, cancellation and completion path.
- **Risk:** A missed release would prevent future recording. Test simultaneous starts from separate connections and hotkeys, plus failures after reservation but before capture begins.

### F7. Jobs report completion even when the result was not stored

- **Kind:** bug
- **Where:** `crates/dettivod/src/history.rs:281`: `if let Err(e) = self.with_store(|store| store.update(item))`, followed only by a warning; `crates/dettivod/src/jobs_run.rs:177`: `jobs.publish(bus, &job_id, progress, last.1, stage);`.
- **Why:** `finish_item` and `finish_meeting` return no persistence result. A database failure leaves the runner free to publish `Done`, remove temporary audio, clear busy state and forget the job. Meeting completion can also start downstream work against an uncommitted result.
- **Change:** Return persistence failures to the runner. Publish success and start downstream processing only after the authoritative result is durable. Retain sufficient recovery material when committing fails.
- **Risk:** Do not leave failed jobs permanently busy. Inject database write failures and verify an honest terminal status, recoverable input and no downstream success callback.

### F8. Transfer files rely on the caller’s umask for confidentiality

- **Kind:** bug
- **Where:** `crates/dettivod/src/transfers.rs:156`: `std::fs::create_dir_all(dir)`; line 165: `std::fs::write(&path, b"")`; line 340: `std::fs::write(&t.path, bytes)`.
- **Why:** Upload staging and rendered exports contain audio or transcript content, but creation specifies no private permissions. Under umask `022`, newly created directories and files can be `0755` and `0644`. Where parent directories are traversable, this bypasses the privacy provided by the socket’s permissions.
- **Change:** Create dedicated transfer directories as `0700` and files as `0600`, using safe file creation. Enforce this in the daemon so standalone execution receives the same protection.
- **Risk:** Existing files and directories need explicit treatment. Check resulting permissions under permissive umasks and verify upload, export and expiry still work.

### F9. Existing connections retain obsolete authentication tokens

- **Kind:** bug
- **Where:** `crates/dettivod/src/server.rs:128`: `let expected_token = daemon.expected_token();`; `crates/dettivod/src/router.rs:84`: `session.daemon.auth_mode()`, paired with `session.expected_token.as_deref()`.
- **Why:** The authentication mode is read for each request, but the expected token is captured when the connection opens. After rotation, an existing connection continues accepting its old token and rejects the new one. Switching from `peer` to `peer_token` can leave existing connections with no usable expected token.
- **Change:** Store mode and resolved token in one authentication snapshot and consult that snapshot per request. Swap it atomically during reload.
- **Risk:** Persistent MCP connections must survive legitimate rotation. Test old and new connections across token replacement and both mode transitions.

### F10. The shutdown timeout does not bound daemon shutdown

- **Kind:** bug
- **Where:** `crates/dettivod/src/server.rs:183`: `router::handle_line(&session, &line)`; `crates/dettivo-session/src/machine.rs:243`: `let _ = w.join();`; `crates/dettivod/src/main.rs:175`: `daemon.engines().shutdown();`.
- **Why:** Socket handlers perform blocking work directly on Tokio workers. Aborting their async tasks cannot interrupt a blocking join or inference call. The configured timeout covers connection draining, while subsequent engine shutdown can wait on locks held by inference. The listener is released before that work finishes.
- **Change:** Dispatch blocking operations through tracked workers, make their cancellation explicit, and apply one deadline to the complete shutdown sequence. Engine termination must be possible without first waiting indefinitely for an inference lock.
- **Risk:** Moving work to `spawn_blocking` alone will not fix cancellation. Test concurrent pings during slow operations and standalone SIGTERM during a deliberately stalled engine call.

### F11. Fixture normalization replaces evidence with the expected answer

- **Kind:** test
- **Where:** `crates/dettivod/tests/contract.rs:274`: `*items = fixture["result"]["items"].as_array().unwrap().clone();`; `crates/dettivo-qa/src/replay.rs:246`: `Value::Array(_) => Value::String("list".into())`; line 250: `_ => Value::String("leaf".into())`.
- **Why:** Search normalization substitutes expected rows for actual rows. An empty search result passes the preceding loop and becomes the expected result. QA normalization discards array contents and scalar types, so missing rows and wrong value types can compare equal.
- **Change:** Deserialize actual responses into protocol types, assert seeded behavior, and normalize only specific nondeterministic fields. Never copy expected result collections into actual results.
- **Risk:** Machine-dependent results need narrow tolerances. Add negative checks proving that empty search results, malformed rows and wrong scalar types fail.

### F12. Strict replay can succeed without exercising implemented stateful methods

- **Kind:** test
- **Where:** `crates/dettivo-qa/src/replay.rs:324`: `STATEFUL.iter().find(...)`, producing `Verdict::Skip`; line 329: `"covered by the daemon's tests"`; `crates/dettivo-qa/src/stateful.rs:90`: `"meetings/notes.set.json"`, excluded because `"updated_at is the daemon's clock"`.
- **Why:** Strict success rejects failures and certain pending rows, but accepts these skips. The replacement coverage is a string assertion rather than an executed link to a test. Several exclusions concern ordinary state setup, generated identifiers or timestamps, so removing their implementation would not make strict replay catch them.
- **Change:** Turn these fixtures into short scenarios with setup requests and captured identifiers, or require machine-checkable evidence from their corresponding integration tests. Preserve explicitly deferred methods admitted by false capability flags.
- **Risk:** Keep model-dependent tests isolated from real model storage. Verify that disabling an advertised stateful handler makes the combined contract gate fail.

### F13. Changing a speech backend leaves the selected recognizer unchanged

- **Kind:** bug
- **Where:** `crates/dettivod/src/engines.rs:132`: `if s.model == model && s.provider == provider { None }`.
- **Why:** Selection identity includes model and provider but omits backend. The recognizer and preloader capture backend when constructed. Changing only `engines.whisper.backend` or `engines.parakeet.backend` therefore leaves subsequent dictation sessions using the previous selection, despite configuration reporting the new value.
- **Change:** Include backend in the selected-engine configuration and rebuild the recognizer and preloader when it changes. Keep an active session’s engine snapshot stable.
- **Risk:** Unnecessary rebuilds could discard useful warm engines. Test a backend-only change with the same provider and model, confirming that the next load receives the new backend and the current session remains consistent.

### F14. Configuration reports values as effective while resources retain old settings

- **Kind:** contract
- **Where:** `docs/config.md:17`: “shows the value in force”; `crates/dettivod/src/history.rs:102`: reload updates only `retention_of(loaded)`; `crates/dettivod/src/logging.rs:13`: “Installs the global subscriber once; later calls are no-ops.”
- **Why:** Database and artifact paths remain those chosen at construction, and the log filter is not reloaded. The socket-path accessor follows the new configuration even though the listener has not moved. Consequently, settings and diagnostics can describe resources the daemon is not using.
- **Change:** Define application timing for each key. Mark storage and listener changes as restart-required and expose that pending state; implement log-filter reload or give it the same treatment. Avoid adding live storage migration merely to support a settings edit.
- **Risk:** Clients must distinguish configured and active values. Test each affected setting before and after restart.

### F15. Configuration validation accepts settings that the pipeline must reject

- **Kind:** bug
- **Where:** `crates/dettivo-core/src/config/validate.rs:54`: `config.rest.bind_error().map(|m| ("rest.bind", m))`; `crates/dettivo-transcribe/src/chunker.rs:60`: `if chunk <= overlap + margin`.
- **Why:** Semantic validation checks only REST binding. Setting `transcribe.chunk_seconds = 1` passes configuration validation, although default overlap and margin are two and five seconds, making every affected transcription fail later. Numeric types also do not enforce documented finite ranges.
- **Change:** Centralize cross-field and range validation in the configuration layer. Validate chunk arithmetic, finite thresholds and closed option sets before writing or installing a configuration.
- **Risk:** Do not reject supported custom model names or legitimate boundary values. Test the same invalid configuration through file loading, `config.validate` and `config.set`, with useful key-specific errors.

### F16. An explicit null request ID is treated as an absent ID

- **Kind:** contract
- **Where:** `crates/dettivo-proto/src/envelope.rs:89`: `pub id: Option<RequestId>`; `crates/dettivod/src/router.rs:81`: `let is_notification = request.id.is_none();`.
- **Why:** Serde maps both an omitted field and JSON `null` to `None` here. Although `RequestId` supports `Null`, the outer option prevents it from preserving an explicit null request ID. The daemon executes that request as a notification and sends no response.
- **Change:** Preserve field presence during deserialization so an explicit null ID receives a null-ID response and an absent ID remains a notification.
- **Risk:** Preserve existing string and numeric IDs and notification behavior. Test all four cases through the live socket; response-only null-ID tests cannot detect this defect.

### F17. Overflow accounting can leave dropped events unreported indefinitely

- **Kind:** bug
- **Where:** `crates/dettivod/src/events.rs:142`: `let n = dropped.swap(0, Ordering::Relaxed);`, followed by `pending.store(false, Ordering::Relaxed)`.
- **Why:** A publisher can drop another event between those operations. It increments the counter but sees an overflow notification already pending, so it schedules nothing. The pending notification sends the earlier count. If publishing then stops, the remaining dropped count is never reported.
- **Change:** Coordinate the dropped count and notification ownership as one state transition, or keep one drain task responsible until the counter is empty.
- **Risk:** Preserve nonblocking publishers and bounded queues. Add a Tokio-runtime test that pauses between the two transitions, causes one final drop, then stops publishing and verifies the complete count.

### F18. Completed connections and successful imports retain bookkeeping forever

- **Kind:** bug
- **Where:** `crates/dettivod/src/server.rs:46`: `tasks.spawn(...)`, with `tasks.join_next()` only in shutdown at line 61; `crates/dettivod/src/imports.rs:279`: `std::mem::forget(self)`.
- **Why:** Finished connection tasks accumulate in the `JoinSet` for the daemon’s entire lifetime. Each successful import also deliberately forgets a guard containing a `String` and `Arc<Jobs>` to suppress its cleanup action. The worker releases the logical import slot, but those allocations remain leaked.
- **Change:** Reap completed tasks in the server’s normal select loop. Give the import guard an explicit disarmed state so transferring responsibility still drops its owned fields.
- **Risk:** Reaping must not accidentally stop acceptance when the set becomes empty. Exercise repeated short connections and imports, checking that retained task and guard counts stabilize.

### F19. Valid request paths violate the stated log-content boundary

- **Kind:** bug
- **Where:** `crates/dettivod/src/transfers.rs:289`: `tracing::debug!(transfer = %id, reason, "transfer cancelled")`; `crates/dettivod/src/handlers/llm.rs:90`: `tracing::info!(endpoint = %canonical, "llm: remote endpoint trusted")`.
- **Why:** `reason` is arbitrary client-provided text, and the endpoint is a configuration value. Both contradict the logging module’s explicit prohibition on request parameters and configuration values. The marker test’s claimed coverage of every request path does not include these valid operations.
- **Change:** Remove the cancellation reason and complete endpoint from logs; retain operation names, counts and controlled status codes. Extend marker testing with successful transfer cancellation and endpoint trust requests.
- **Risk:** Preserve useful diagnostics without copying user content. Run valid requests containing unique markers at trace level and assert that none reaches captured logs.

### F20. Delete the redundant dispatch inventory and duplicate parameter decoding

- **Kind:** simplify
- **Where:** `crates/dettivod/src/handlers/mod.rs:58`: `pub const IMPLEMENTED: &[&str]`; line 158: `match method`; `crates/dettivod/src/router.rs:138`: `catalog::round_trip_params(spec.name, params)`.
- **Why:** Implemented method names are maintained separately from the actual dispatcher and protocol catalog. Requests are deserialized and serialized by the catalog, then deserialized again by handlers. Repeated handler-local decoding and JSON helpers add maintenance points without adding another meaningful boundary.
- **Change:** Derive implementation availability from the dispatcher, delete the separate name list, and decode parameters once. Share the small response/error helpers while preserving protocol types and field-specific diagnostics. A generic handler framework is unnecessary.
- **Risk:** Refactoring can alter error details or advertised method availability. Compare dispatch coverage with the catalog and pin representative invalid-parameter responses before removing the redundant paths.

## Keep

- `crates/dettivod/src/router.rs`: one daemon-side routing and authentication boundary shared by clients is the right architecture.
- `crates/dettivo-core/src/config/edit.rs`: comment-preserving file edits support configuration as the source of truth.
- `crates/dettivo-core/src/config/keys.rs`: the central key registry is worth extending with validation and application timing.
- `crates/dettivo-proto/src/catalog.rs`: retain the typed contract inventory; repair how execution tests use it.
- `crates/dettivo-speech/src/supervisor.rs`: engine process isolation remains appropriate for crash containment and independent engine lifecycles.
- `crates/dettivod/src/events.rs`: bounded delivery with nonblocking publishers is appropriate; fix its overflow coordination.
- `crates/dettivod/src/platform.rs`: truthful false capability flags for deferred features should remain false until implemented.
- The 129 Rust files inspected in the three requested crates meet the 500-line limit. Further splitting solely for size would add work without benefit.

## Questions for the owner

- When an existing configuration is invalid at cold start, should the daemon refuse startup or offer a restricted repair mode? I recommend refusing startup with a precise diagnostic.
- Is restart-required behavior acceptable for storage and socket-path changes? I recommend it over implementing live migration.

Verification: the checkout remains unchanged, and all 136 JSON fixtures parsed successfully. I did not run `just build test lint` because it writes build and test artifacts; these findings are based on source analysis, not runtime reproductions.