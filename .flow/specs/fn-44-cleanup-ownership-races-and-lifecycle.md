# Cleanup: ownership, races and lifecycle (23 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

Every session, job, download and connection has one owner from start to durable end; no control path blocks behind inference; timeouts bound the whole operation and leave a known state. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0046 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F2** (bug): A second daemon modifies the first daemon’s work before refusing to start
  - Where: `crates/dettivod/src/main.rs:112`: `daemon.meetings().recover_stale(daemon.history())`, before line 123’s `listener::acquire(&daemon).await`; `crates/dettivod/src/history.rs:77`: `store.fail_stale_jobs("the daemon restarted before the job finished")?`.
  - Why: Construction opens and mutates the database before checking whether another instance owns the socket. Recovery assumes all unfinished jobs and recordings belong to a dead process. A duplicate launch can therefore mark active imports failed and recover a meeting that the first daemon is still recording.
  - Change: Acquire exclusive ownership of the data store before migrations or recovery. Begin accepting requests only after recovery completes. Socket selection alone should not permit two daemons to recover the same database.
  - Risk: Preserve socket activation and genuine crash recovery. Launch a second daemon during recording and importing; verify that it exits without changing rows or audio files.

- **daemon/F5** (bug): Analysis cancellation and completion race with deletion
  - Where: `crates/dettivod/src/analysis.rs:291`: `.remove(&row.id)`, before line 302’s `s.set_meeting_analysis(...)`; `crates/dettivod/src/handlers/meetings_recovery.rs:31`: `daemon.analysis().cancel(id)`.
  - Why: Completion removes the running job before committing its result. Deletion in that interval sees nothing to cancel, clears the transcript and analysis, then the old worker restores the analysis. Admission has a related gap: `start` checks for an existing job separately from inserting the new one, permitting duplicate analyses whose cleanup can remove each other’s registration.
  - Change: Use one per-meeting lifecycle guard for admission, cancellation and final publication. Check job identity when completing; keep ownership until database and file publication finish.
  - Risk: Preserve cancellation responsiveness without holding a lock during inference. Test simultaneous starts, cancellation followed by restart, and deletion paused immediately before result commit.

- **daemon/F6** (bug): Dictation and meeting starts can both pass their exclusion checks
  - Where: `crates/dettivod/src/actions.rs:78`: `if daemon.meetings().snapshot().is_some()`; `crates/dettivod/src/handlers/meetings.rs:179`: `if daemon.dictation().snapshot().is_some()`.
  - Why: Each route checks the other session machine, then starts its own independently. Two concurrent connections can both observe idle state and proceed. The individual session locks prevent duplicate sessions of one kind, but do not enforce the advertised exclusion between kinds.
  - Change: Introduce one daemon-owned capture reservation shared by dictation, meetings and hotkey starts. Acquire it before setup and release it on every failure, cancellation and completion path.
  - Risk: A missed release would prevent future recording. Test simultaneous starts from separate connections and hotkeys, plus failures after reservation but before capture begins.

- **daemon/F9** (bug): Existing connections retain obsolete authentication tokens
  - Where: `crates/dettivod/src/server.rs:128`: `let expected_token = daemon.expected_token();`; `crates/dettivod/src/router.rs:84`: `session.daemon.auth_mode()`, paired with `session.expected_token.as_deref()`.
  - Why: The authentication mode is read for each request, but the expected token is captured when the connection opens. After rotation, an existing connection continues accepting its old token and rejects the new one. Switching from `peer` to `peer_token` can leave existing connections with no usable expected token.
  - Change: Store mode and resolved token in one authentication snapshot and consult that snapshot per request. Swap it atomically during reload.
  - Risk: Persistent MCP connections must survive legitimate rotation. Test old and new connections across token replacement and both mode transitions.

- **daemon/F10** (bug): The shutdown timeout does not bound daemon shutdown
  - Where: `crates/dettivod/src/server.rs:183`: `router::handle_line(&session, &line)`; `crates/dettivo-session/src/machine.rs:243`: `let _ = w.join();`; `crates/dettivod/src/main.rs:175`: `daemon.engines().shutdown();`.
  - Why: Socket handlers perform blocking work directly on Tokio workers. Aborting their async tasks cannot interrupt a blocking join or inference call. The configured timeout covers connection draining, while subsequent engine shutdown can wait on locks held by inference. The listener is released before that work finishes.
  - Change: Dispatch blocking operations through tracked workers, make their cancellation explicit, and apply one deadline to the complete shutdown sequence. Engine termination must be possible without first waiting indefinitely for an inference lock.
  - Risk: Moving work to `spawn_blocking` alone will not fix cancellation. Test concurrent pings during slow operations and standalone SIGTERM during a deliberately stalled engine call.

- **daemon/F13** (bug): Changing a speech backend leaves the selected recognizer unchanged
  - Where: `crates/dettivod/src/engines.rs:132`: `if s.model == model && s.provider == provider { None }`.
  - Why: Selection identity includes model and provider but omits backend. The recognizer and preloader capture backend when constructed. Changing only `engines.whisper.backend` or `engines.parakeet.backend` therefore leaves subsequent dictation sessions using the previous selection, despite configuration reporting the new value.
  - Change: Include backend in the selected-engine configuration and rebuild the recognizer and preloader when it changes. Keep an active session’s engine snapshot stable.
  - Risk: Unnecessary rebuilds could discard useful warm engines. Test a backend-only change with the same provider and model, confirming that the next load receives the new backend and the current session remains consistent.

- **daemon/F14** (contract): Configuration reports values as effective while resources retain old settings
  - Where: `docs/config.md:17`: “shows the value in force”; `crates/dettivod/src/history.rs:102`: reload updates only `retention_of(loaded)`; `crates/dettivod/src/logging.rs:13`: “Installs the global subscriber once; later calls are no-ops.”
  - Why: Database and artifact paths remain those chosen at construction, and the log filter is not reloaded. The socket-path accessor follows the new configuration even though the listener has not moved. Consequently, settings and diagnostics can describe resources the daemon is not using.
  - Change: Define application timing for each key. Mark storage and listener changes as restart-required and expose that pending state; implement log-filter reload or give it the same treatment. Avoid adding live storage migration merely to support a settings edit.
  - Risk: Clients must distinguish configured and active values. Test each affected setting before and after restart.

- **daemon/F18** (bug): Completed connections and successful imports retain bookkeeping forever
  - Where: `crates/dettivod/src/server.rs:46`: `tasks.spawn(...)`, with `tasks.join_next()` only in shutdown at line 61; `crates/dettivod/src/imports.rs:279`: `std::mem::forget(self)`.
  - Why: Finished connection tasks accumulate in the `JoinSet` for the daemon’s entire lifetime. Each successful import also deliberately forgets a guard containing a `String` and `Arc<Jobs>` to suppress its cleanup action. The worker releases the logical import slot, but those allocations remain leaked.
  - Change: Reap completed tasks in the server’s normal select loop. Give the import guard an explicit disarmed state so transferring responsibility still drops its owned fields.
  - Risk: Reaping must not accidentally stop acceptance when the set becomes empty. Exercise repeated short connections and imports, checking that retained task and guard counts stabilize.

### agent-surfaces

- **agent-surfaces/F4** (contract): Invalid MCP arguments can select the wrong session to cancel
  - Where: `crates/dettivo-mcp/src/protocol.rs:208` — arguments are filtered with `.filter(|a| a.is_object())` and replaced with `{}`; `crates/dettivo-mcp/src/tools/dispatch.rs:199` — `client.call(&format!("dictation.{action}"), json!({}))`.
  - Why: Non-object arguments become an empty argument object. A supplied numeric `meeting_id` also disappears through `str_arg`, routing `cancel_session` to dictation instead of rejecting the malformed meeting request. The published schemas do not validate execution.
  - Change: Reject supplied arguments of the wrong type before applying defaults or selecting a daemon method. Keep “absent” distinct from “present but invalid.”
  - Risk: Preserve documented defaults and intentional coercions such as insertion PID conversion. Test malformed cancellation requests with a recording dictation and assert that no daemon mutation occurs.

- **agent-surfaces/F7** (bug): REST retains every completed connection task until shutdown
  - Where: `crates/dettivo-rest/src/server.rs:135` — `tasks.spawn(serve(...))`; `crates/dettivo-rest/src/server.rs:147` — `while tasks.join_next().await.is_some() {}` occurs only after the accept loop ends.
  - Why: Completed tasks remain in the `JoinSet` until joined. A resident daemon accumulates task records as connections come and go, including unauthenticated connections.
  - Change: Reap completed tasks inside the accept loop. Bound active connections so an unauthenticated peer cannot create unlimited concurrent request buffers.
  - Risk: Exercise many short connections and verify that retained task count returns to zero while the listener remains running.

- **agent-surfaces/F8** (bug): REST’s request timeout leaves work running and does not bound the whole request
  - Where: `crates/dettivo-rest/src/server.rs:157` — timeout around `read_request`; `crates/dettivo-rest/src/server.rs:185` — a fresh timeout around `spawn_blocking`; `crates/dettivo-rest/src/server.rs:198` — response writing has no timeout.
  - Why: Reading and handling each receive the full budget, and writing can block indefinitely. Timing out the blocking task drops its handle without stopping it, so transfer steps or mutations can continue after HTTP reports failure. A retry can overlap the original operation.
  - Change: Carry one deadline through reading, dispatch and writing. Stop scheduling further transfer steps after expiry, and distinguish an unknown mutation outcome from a confirmed failure. Keep blocking work tracked through completion.
  - Risk: Use a backend that pauses before a mutation, a slow upload and a client that stops reading. Verify deadlines, shutdown behavior and whether late mutations occur.

### engines

- **engines/F1** (bug): Download completion and model status can deadlock each other
  - Where: `crates/dettivo-speech/src/download.rs:114` holds `let mut g = p.lock()` through `on_progress(&g)` at line 122. `crates/dettivod/src/models.rs:287` then takes `inner.lock()`, while line 342 calls `d.progress()` from the locked model service.
  - Why: Completion takes the progress lock followed by the model-service lock. A concurrent status request takes those locks in reverse order. Each can wait indefinitely for the other.
  - Change: Update progress and clone its final snapshot under the lock, release the guard, then invoke the callback. Never call external callbacks while holding the progress lock.
  - Risk: Preserve the terminal progress event. Test completion racing with status using barriers that force the opposing lock order.

- **engines/F3** (bug): Timeouts neither bound the complete operation nor invalidate uncertain engine state
  - Where: `crates/dettivo-speech/src/process.rs:215` calls `write_frame(...)` before creating the deadline at line 216; line 267 returns a transport error without terminating the process. `crates/dettivo-speech/src/supervisor.rs:208` returns other load errors without clearing the slot.
  - Why: Waiting for the slot, loading and blocking pipe writes sit outside the request timeout. A timed-out engine remains available for reuse. Worse, if loading model B times out while the cached model is A, B can finish loading later; a subsequent request for A can skip loading and run against B.
  - Change: Carry one deadline through queueing, loading, writing and response handling. After a timeout or broken protocol, terminate the child and clear cached load state unless cancellation and recovery have been positively acknowledged.
  - Risk: Recovery will sacrifice warm models. Test stalled stdin, ignored cancellation, queued requests and a late model-load response.

- **engines/F6** (contract): Changing load settings can silently retain the old configuration
  - Where: `crates/dettivo-speech/src/supervisor.rs:177` compares only `l.model != model || s.lora.as_deref() != params.lora.as_deref()`. `crates/dettivod/src/engines.rs:132` rebuilds the selected speech engine only when its model or provider changes.
  - Why: Backend preference, context length, VAD model and diarization thread count are absent from the cache identity. A backend-only speech configuration change also leaves the old engine wrapper intact. A successful configuration write can therefore have no effect.
  - Change: Cache the complete effective load configuration. Rebuild speech wrappers when their backend changes, and invalidate processes when their executable selection changes.
  - Risk: Configuration reloads may trigger additional loads. Test each load-affecting setting independently while keeping the model path constant.

### dictation

- **dictation/F1** (bug): Cancellation during polishing can still insert and archive the transcript
  - Where: `crates/dettivo-session/src/worker.rs:126` — `if self.cancel.load(Ordering::Relaxed)`; `crates/dettivo-session/src/worker.rs:168` — `dettivo_language::pipeline::run(`; `crates/dettivo-session/src/worker.rs:202` — `Some(self.inserter.insert(&text, self.target.as_ref()))`.
  - Why: The worker checks cancellation after transcription, then runs the language pipeline and proceeds directly to insertion. Cancellation during a slow rewrite therefore does not prevent insertion or archival. Meanwhile, `crates/dettivo-session/src/machine.rs:279` returns a snapshot whose state is `State::Cancelled`, even if the worker completed those effects.
  - Change: Carry cancellation through language processing and insertion preparation. Coordinate cancellation with the final insertion decision, and distinguish cancellation before delivery from an operation whose delivery has already started.
  - Risk: A check alone leaves a race immediately before typing. Test cancellation with a provider blocked behind a barrier, then release it and assert no insertion, archive or successful completion.

- **dictation/F8** (bug): Cancellation hotkeys can be delayed until after insertion—or discarded entirely
  - Where: `crates/dettivod/src/hotkeys.rs:121` — `for event in rx`; `crates/dettivod/src/actions.rs:153` — `Release(Action::PushToTalk) => stop(daemon)`; `crates/dettivo-hotkeys/src/evdev.rs:45` — `if self.active.is_some() { return None; }`.
  - Why: The portal/evdev action consumer calls synchronous stop, which waits through transcription, rewriting and insertion. A subsequent cancel event remains queued until that finishes. Separately, evdev rejects any second chord while the push-to-talk chord is held, including cancellation during recording.
  - Change: Keep hotkey dispatch responsive by separating the stop signal from completion waiting. Allow the cancel action while push-to-talk remains active without losing the eventual release event.
  - Risk: Preserve event ordering and prevent duplicate starts. Test cancellation while holding push-to-talk and after release while a fake engine or provider is blocked.

- **dictation/F15** (bug): The rewrite timeout does not bound provider discovery or credential lookup
  - Where: `crates/dettivo-language/src/pipeline.rs:149` — provider resolution precedes rewriting; `crates/dettivo-language/src/enhanced/mod.rs:131` — `let deadline = Instant::now() + timeout`; `crates/dettivo-language/src/provider/mod.rs:201` — `Command::new("secret-tool")...output()`.
  - Why: Provider probing occurs before the rewrite deadline starts. Credential lookup can block outside the HTTP request timeout, and the helper subprocess has no deadline. Thus the configured budget does not bound release-to-result time.
  - Change: Establish one deadline before provider resolution and pass its remaining budget through probes, credential lookup and inference. Bound and cancel subprocesses as well as network requests.
  - Risk: A short budget may be exhausted during discovery. Test delayed probes and a stalled credential helper, asserting bounded completion and deterministic fallback.

### meetings

- **meetings/F13** (bug): Restart leaves analysis and diarization permanently marked running
  - Where: `crates/dettivod/src/history.rs:75` — startup settles transcription jobs only; `crates/dettivod/src/analysis.rs:248` — persists `AnalysisStatus::Running`; `crates/dettivod/src/diarization.rs:354` — persists `DiarizationStatus::Running`.
  - Why: The running maps disappear with the process, but their persisted statuses receive no restart reconciliation. A completed meeting remains “running” without a worker. The UI disables both retry buttons based on those statuses in `qt/qml/Dettivo/app/meetings/AnalysisView.qml:33` and line 43.
  - Change: On startup, settle orphaned analysis and diarization attempts with an interrupted reason while preserving previous successful results.
  - Risk: Restart must not erase an earlier analysis or speaker assignment. Kill the daemon during each pass and verify the record remains readable and both retries become available.

### qa-rig

- **qa-rig/F17** (contract): Scenario timeouts do not bound the operations they wrap
  - Where: `crates/dettivo-qa/src/driver/cua.rs:83`: `.output()`; `crates/dettivo-qa/src/socket.rs:20`: `stream.set_read_timeout(Some(timeout))?;`; `crates/dettivo-qa/src/scenarios/daemon.rs:208`: `Ok(0) | Err(_) => break`.
  - Why: A blocking cua subprocess can outlive every surrounding deadline. Socket timeouts cover individual reads, not the complete request. Event collection uses a separately configured read timeout and converts transport failures into an ordinary partial event list.
  - Change: Give subprocess and socket operations absolute deadlines, pass remaining time into blocking operations, and return explicit timeout/transport errors with partial evidence.
  - Risk: Model loading needs longer budgets than UI actions. Test a hung subprocess, a slowly trickling response, and a silent event stream without launching real applications.

### qa-packs

- **qa-packs/F13** (bug): An import error leaves the GPU sampler running
  - Where: `crates/dettivo-qa/src/pack/meeting_throughput.rs:314` — `let sampler = gpu_proof::Sampler::start(...)`; `crates/dettivo-qa/src/pack/meeting_throughput.rs:324` — `)?`; `crates/dettivo-qa/src/pack/gpu_proof.rs:196` — `pub fn stop(mut self) -> Samples`.
  - Why: Errors from the import call or missing response identifiers return before `stop()`. `Sampler` has no drop cleanup. Dropping its join handle detaches the thread, which retains its stop flag and continues polling and accumulating samples during later pack steps.
  - Change: Give the sampler unconditional cleanup that stops and joins its thread, including early returns. Bound external counter calls so cleanup cannot wait forever.
  - Risk: Cleanup must avoid double joins and preserve useful partial evidence. Inject an import failure immediately after sampler creation and verify polling stops.

### qt-hosts

- **qt-hosts/F9** (bug): Concurrent launches can create two apps and steal each other’s control socket
  - Where: `qt/host/app/app_control.cpp:69`, `QLocalServer::removeServer(m_path);`; `qt/apps/dettivo-app/app_main.cpp:165`, `AppControl::forward(...)`, with `control.listen(...)` deferred until line 403.
  - Why: There is a substantial gap between checking for an existing app and claiming the socket. Two launches can both pass the check, initialize their windows and then unlink each other’s listening socket. A slow existing app can also be mistaken for a crashed one. The destructor subsequently removes the pathname without establishing that it still belongs to this instance.
  - Change: Acquire exclusive instance ownership before GUI initialization. Only the owner may remove a stale socket; other launches should forward or wait for startup.
  - Risk: Exercise simultaneous cold launches, slow startup, a stale socket and an unresponsive live instance. Exactly one window and one usable control endpoint should remain.

- **qt-hosts/F12** (bug): Requests can remain pending forever while the socket still appears connected
  - Where: `qt/host/daemon/daemon_client.cpp:102`, `m_pending.insert(id, std::move(reply));`; line 103 calls `m_socket.write(...)` without checking its result.
  - Why: Pending callbacks complete only on a matching reply or disconnection. There is no deadline. A daemon that accepts the socket but stops answering leaves actions permanently busy and loading indicators unresolved. Periodic reads can continue accumulating callbacks.
  - Change: Add request deadlines and handle write failures through the same completion path. Expiry must fail each callback exactly once; it must not automatically replay mutations.
  - Risk: A timed-out mutation may already have succeeded. Report that uncertainty and refresh its state. Test an accepted-but-silent connection, late replies and disconnect during expiry.

### ops-and-record

- **ops-and-record/F11** (bug): Reconnecting the plugin does not restore active sessions
  - Where: `omarchy/DettivoState.qml:280` only handles `!status.is_active`; line 258 resets dictation to `"idle"` after stream exit.
  - Why: An active status response never restores recording or transcribing. There is no corresponding meeting snapshot request. Reloading the shell during a session can leave the bar showing ready until another transition arrives; audio-level events cannot repair that state.
  - Change: Restore dictation and meeting state from snapshots after subscription/reconnection, including the meeting ID. Define ordering so a late snapshot cannot overwrite a newer event.
  - Risk: Snapshot/event races. Reload during recording and transcription, and interrupt the event stream during a meeting.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F2, A second daemon modifies the first daemon’s work before refusing to start: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** daemon/F5, Analysis cancellation and completion race with deletion: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** daemon/F6, Dictation and meeting starts can both pass their exclusion checks: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** daemon/F9, Existing connections retain obsolete authentication tokens: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** daemon/F10, The shutdown timeout does not bound daemon shutdown: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** daemon/F13, Changing a speech backend leaves the selected recognizer unchanged: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** daemon/F14, Configuration reports values as effective while resources retain old settings: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** daemon/F18, Completed connections and successful imports retain bookkeeping forever: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** agent-surfaces/F4, Invalid MCP arguments can select the wrong session to cancel: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** agent-surfaces/F7, REST retains every completed connection task until shutdown: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** agent-surfaces/F8, REST’s request timeout leaves work running and does not bound the whole request: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** engines/F1, Download completion and model status can deadlock each other: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** engines/F3, Timeouts neither bound the complete operation nor invalidate uncertain engine state: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** engines/F6, Changing load settings can silently retain the old configuration: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** dictation/F1, Cancellation during polishing can still insert and archive the transcript: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** dictation/F8, Cancellation hotkeys can be delayed until after insertion—or discarded entirely: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** dictation/F15, The rewrite timeout does not bound provider discovery or credential lookup: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** meetings/F13, Restart leaves analysis and diarization permanently marked running: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R19:** qa-rig/F17, Scenario timeouts do not bound the operations they wrap: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R20:** qa-packs/F13, An import error leaves the GPU sampler running: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R21:** qt-hosts/F9, Concurrent launches can create two apps and steal each other’s control socket: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R22:** qt-hosts/F12, Requests can remain pending forever while the socket still appears connected: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R23:** ops-and-record/F11, Reconnecting the plugin does not restore active sessions: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R24:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0046 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
