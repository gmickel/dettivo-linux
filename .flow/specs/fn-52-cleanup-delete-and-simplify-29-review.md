# Cleanup: delete and simplify (29 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

What the reviewers found unnecessary or duplicated is gone, and what remains is the simplest thing that keeps the behaviour the specs require. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0054 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F20** (simplify): Delete the redundant dispatch inventory and duplicate parameter decoding
  - Where: `crates/dettivod/src/handlers/mod.rs:58`: `pub const IMPLEMENTED: &[&str]`; line 158: `match method`; `crates/dettivod/src/router.rs:138`: `catalog::round_trip_params(spec.name, params)`.
  - Why: Implemented method names are maintained separately from the actual dispatcher and protocol catalog. Requests are deserialized and serialized by the catalog, then deserialized again by handlers. Repeated handler-local decoding and JSON helpers add maintenance points without adding another meaningful boundary.
  - Change: Derive implementation availability from the dispatcher, delete the separate name list, and decode parameters once. Share the small response/error helpers while preserving protocol types and field-specific diagnostics. A generic handler framework is unnecessary.
  - Risk: Refactoring can alter error details or advertised method availability. Compare dispatch coverage with the catalog and pin representative invalid-parameter responses before removing the redundant paths.

### agent-surfaces

- **agent-surfaces/F10** (simplify): Compositor dictation overrides configured mode and implements toggle outside the daemon
  - Where: `crates/dettivo-cli/src/commands.rs:269` — absent mode becomes `"raw"`; `crates/dettivo-cli/src/commands.rs:279` — `dictation.status` precedes a separate start or stop; `crates/dettivod/src/dictation.rs:181` — omitted mode otherwise resolves from `d.mode`.
  - Why: Generated compositor bindings call the CLI without a mode, so they force raw mode while daemon-side hotkeys can use the configured mode. Toggle also makes a decision from a snapshot that can change before its next request.
  - Change: Give compositor actions a daemon-owned operation that resolves configured defaults and serializes the toggle decision. Delete the CLI’s status-then-action branch. Preserve explicitly documented MCP defaults separately.
  - Risk: Test non-raw configuration through compositor, portal and evdev paths, plus concurrent toggles and explicit mode overrides.

- **agent-surfaces/F12** (simplify): Three transfer implementations buffer whole files and disagree about cleanup
  - Where: `crates/dettivo-cli/src/transfer.rs:98` — `let mut bytes = Vec::new()`; `crates/dettivo-mcp/src/tools/transfer.rs:151` — the same accumulation; `crates/dettivo-rest/src/stream.rs:182` — `let mut body = Vec::new()`; `crates/dettivo-mcp/src/tools/transfer.rs:77` — upload errors return directly through `?`.
  - Why: Chunked transfers still materialize entire downloads in each client. CLI and MCP also read whole uploads before transferring them. Cleanup differs: MCP export cancels on failure, MCP import does not, and CLI transfers return from several failure paths without cancellation.
  - Change: Replace the three loops with shared transfer primitives over readers and writers, incremental hashing, and one explicit cancellation path. Stream file output through a temporary destination; retain only the bounded preview MCP needs.
  - Risk: Check large-file memory use, interrupted uploads, disk-full writes, failed imports and failed final acknowledgments. Preserve existing output files until download completion.

- **agent-surfaces/F14** (simplify): Doctor can report success when required probes failed
  - Where: `crates/dettivo-cli/src/doctor.rs:139` — configuration errors become an “unreachable” note without clearing `healthy`; `crates/dettivo-cli/src/doctor.rs:152` — engine errors become `Null`; `crates/dettivo-cli/src/doctor.rs:168` — LLM engine errors also become `Null`.
  - Why: After the initial health probe succeeds, later failures can leave the final exit code at zero. Error classes are also lost or mislabeled as daemon unavailability. Doctor’s 927 lines across collection and rendering contain repeated model summaries, engine presentation and client facts.
  - Change: Track success, failure and unknown explicitly for each required probe. Move daemon-owned diagnostic aggregation behind the daemon contract, retain local service/socket checks in the CLI, and reduce the human report to failures plus concise readiness rows.
  - Risk: Preserve the documented JSON surface during migration. Test failures after a successful health response, including authorization errors and engine-query timeouts.

- **agent-surfaces/F15** (delete): Delete two copies of the Unix-socket client
  - Where: `crates/dettivo-cli/src/client.rs:61`, `crates/dettivo-mcp/src/client.rs:117` and `crates/dettivo-rest/src/backend.rs:66` each implement `fn call`, connection setup, authentication, serialization and response parsing.
  - Why: The same transport is maintained three times, with different error representations and timeout mappings. All three read an unbounded response line. CLI subscription setup adds another variant that sets only a write timeout before waiting for its initial response.
  - Change: Keep one shared transport with typed transport failures, explicit response bounds and request deadlines. Leave CLI exit codes, MCP guidance and REST status mapping in their adapters.
  - Risk: Preserve existing error text and authentication precedence where contractual. Test EOF, malformed responses, delayed subscription acknowledgment, oversized responses and slowly delivered responses.

### engines

- **engines/F9** (simplify): Model replacement temporarily keeps both models resident
  - Where: `crates/dettivo-engine-proto/src/host_llm.rs:125` calls `E::load` before replacing `self.engine` at line 128. `crates/dettivo-engine-llm/src/engine.rs:164` compares the new model’s size with current device free memory.
  - Why: The old model still consumes memory during the new load. Two models that each fit individually can fail to switch on Vulkan or force an unnecessary CPU fallback. Speech and diarization hosts use the same replacement pattern.
  - Change: Unload the previous model before loading its replacement. A failed replacement should leave an explicit unloaded state. Reset Parakeet’s process-global backend only after its old model has been dropped.
  - Risk: Failed switches lose the previous warm model. Test switching between two models that fit separately but not simultaneously.

- **engines/F16** (delete): The transcription crate carries an unused language dependency
  - Where: `crates/dettivo-transcribe/Cargo.toml:14` declares `dettivo-language.workspace = true`. Its only source reference is `dettivo_language::CRATE_NAME` in `crates/dettivo-transcribe/src/lib.rs:31`.
  - Why: Transcription uses no language-crate behavior. The dependency exists solely to make a declared dependency list resolve, adding coupling without functionality.
  - Change: Delete the dependency and its `UPSTREAM` entry; update the corresponding dependency-edge expectation.
  - Risk: No runtime behavior should change. Run the transcription tests and dependency-edge lint.

### dictation

- **dictation/F4** (delete): Delete the second transcript archive outside history retention
  - Where: `crates/dettivo-session/src/worker.rs:230` — `write_transcript(&self.dir, &transcript)`; `crates/dettivo-session/src/worker.rs:374` — `serde_json::to_string_pretty(t)`; `crates/dettivo-session/src/worker.rs:362` — `if !self.policy.keep_audio`.
  - Why: Every successful take writes the complete transcript object, including raw text and frozen policy, into its session directory before separately archiving it. Cleanup only discards take files when audio retention is disabled; `crates/dettivo-audio/src/takes.rs:178` does not remove this transcript file. History retention owns a different artifact directory. Cancelled takes can also retain audio through the same `keep_audio` condition without creating a history item.
  - Change: Remove the redundant transcript write. Give history sole ownership of retained artifacts, clean successful staging after archival, and discard cancelled takes regardless of successful-take retention preferences.
  - Risk: Preserve recoverability when archival fails through an explicit, bounded recovery mechanism. Check raw-disabled history, history deletion, retention expiry and cancellation against both history and session storage.

- **dictation/F5** (delete): Delete synthesized undo until it can identify the inserted range
  - Where: `crates/dettivo-insert/src/service.rs:169` — `then.app_id == now.app_id && then.pid == now.pid`; `crates/dettivo-insert/src/backend/virtual_keyboard.rs:310` — `typist.combo(&[(key("Shift_L"), shift)], &[key("Left")], chars)` followed by `typist.tap(key("Delete"))`.
  - Why: Matching the process within a short time window does not establish that the cursor remains immediately after the inserted text. Typing more text, moving the cursor or switching windows can make this delete unrelated content. Unicode scalar counts also need not equal cursor movements.
  - Change: Stop advertising undo for backends that only synthesize selection and deletion. Restore it only where the backend can verify the inserted range or use a suitable application transaction.
  - Risk: This removes a convenience but avoids destructive guesses. Cover cursor movement, intervening typing, same-process windows, combining characters and terminal behavior before restoring support.

- **dictation/F13** (simplify): Disabled transforms and post-processors still run through unconditional passes
  - Where: `crates/dettivo-language/src/polish/mod.rs:43` — punctuation runs for `SmartPunctuation || FixGrammar`; `crates/dettivo-language/src/polish/post.rs:34` — selected processors run before unconditional normalization at lines 41–52.
  - Why: Disabling Smart Punctuation does not prevent punctuation when Fix Grammar remains enabled. Selecting any post-processor also brings along unrelated fixed passes, some duplicating selected processors. The Code preset filters out Smart Punctuation but still reaches capitalization and terminal punctuation.
  - Change: Delete duplicated passes and make each configurable operation run once, only when enabled. Separate genuinely mandatory normalization from optional editorial changes, and update the parity decision to describe the resulting behavior.
  - Risk: This intentionally changes some macOS parity outputs. Check disabled-transform behavior, processor subsets, literal commands and paragraph preservation; resolve the parity question below before changing defaults.

### meetings

- **meetings/F3** (delete): Delete the redundant meeting-row copy from metadata
  - Where: `crates/dettivo-meeting/src/worker_end.rs:262` — `"meeting": row`; `crates/dettivo-meeting/src/lib.rs:60` — `metadata["meeting"] = serde_json::to_value(row)`; `crates/dettivod/src/handlers/meetings_recovery.rs:37` — the transcript-only branch clears the database and removes only the analysis file.
  - Why: The metadata sidecar duplicates transcript, segments, notes and potentially analysis from the database. Transcript deletion leaves this copy untouched; a partial meeting’s checkpoint can retain another transcript copy. The delete test cannot detect that leak because `crates/dettivod/tests/meetings_delete.rs:38` writes filenames as placeholder file contents.
  - Change: Remove the duplicated row block and its rewrite machinery; retain the capture manifest needed for recovery. Clear transcript-bearing checkpoints when deleting a transcript. If the full metadata document must remain supported, regenerate it from the cleared row.
  - Risk: External consumers may read the metadata document. Check those consumers and test deletion using real populated metadata and checkpoints, asserting that deleted text is absent from every retained artifact.

- **meetings/F10** (simplify): Collapse the completion hooks into one ordered post-processing path
  - Where: `crates/dettivo-meeting/src/worker_end.rs:236` — audio is removed when `!policy.keep_audio`; `crates/dettivod/src/meetings.rs:102` — the completed event invokes the speaker hook; `crates/dettivod/src/analysis.rs:309` — analysis writes its sidecar unconditionally.
  - Why: Audio retention runs before the completed event starts diarization, so automatic speaker assignment cannot work when audio retention is disabled. With artifacts disabled, successful analysis recreates a meeting directory that finalisation just removed. Two independent completion mechanisms obscure these dependencies.
  - Change: Give one completion owner responsibility for automatic post-processing and retention. Keep temporary audio until the speaker pass finishes, then apply retention; honor artifact policy when writing analysis and metadata.
  - Risk: Failed or unavailable post-processing must not retain temporary audio indefinitely. Test automatic analysis and diarization across the audio/artifact policy combinations.

- **meetings/F15** (simplify): Status polling unnecessarily loads the entire meeting archive
  - Where: `crates/dettivo-storage/src/meetings.rs:147` — `SELECT {COLUMNS} FROM meetings ORDER BY created_at, seq`; line 154 — status filtering happens in Rust; `crates/dettivod/src/handlers/meetings.rs:432` — every status response calls `recoverable(daemon)?`.
  - Why: Each poll deserializes every meeting’s transcript, notes and analysis just to identify partial meetings. List and search also materialize the full wide row and attach speakers individually before discarding most fields. All of this runs under the shared store lock.
  - Change: Filter statuses in SQL and introduce small read projections for status, recovery lists, search hits and list rows. Load the full meeting only for detail and export.
  - Risk: Summary projections must preserve wire fields and cursor ordering. Test a large archive containing long transcripts and maximum-size notes; inspect rows read and response latency.

### qa-rig

- **qa-rig/F18** (simplify): Process cleanup depends on reaching the runner’s normal teardown
  - Where: `crates/dettivo-qa/src/scenarios/daemon.rs:74`: readiness timeout returns before `DaemonHandle` construction; `crates/dettivo-qa/src/profile.rs:164`: `self.children.push((name.to_string(), child.id()));`; `crates/dettivo-qa/src/profile.rs:224`: profile drop removes its directory.
  - Why: The profile retains PIDs rather than child ownership. Failed startup drops an unguarded `Child`; callers outside the scenario runner, including pack platform probing, can leave it running while deleting its profile. Other paths rely on duplicate driver and runner cleanup.
  - Change: Construct a process guard immediately after spawn. Consolidate termination and reaping under explicit ownership, including startup failure, and remove stale PID-only cleanup.
  - Risk: Avoid double termination with existing driver guards. Test a daemon that starts but never answers readiness; no child or zombie should remain after either caller exits.

- **qa-rig/F19** (delete): Delete the first-run scenario’s second model-store lifecycle
  - Where: `crates/dettivo-qa/src/scenarios/first_run_support.rs:167`: `repo_root.join("target/qa-first-run").join(name)`; `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:126`: `let _ = std::fs::remove_dir_all(store);`.
  - Why: First-run creates another store and symlink hierarchy despite the profile already owning model storage. Manual removal ignores `--keep-profile`, while failures between staging and the drive bypass removal.
  - Change: Stage the tiny model and downloadable test model through the existing profile model tree. Keep the scenario-specific catalogue/server; delete the second store, extra link, configuration redirection, and manual cleanup.
  - Risk: The download target must begin missing. Verify fresh download, interrupted setup, normal cleanup, and retained-profile inspection.

### qa-packs

- **qa-packs/F6** (simplify): The visual comparator deliberately ignores regressions the gate promises to catch
  - Where: `qt/tools/visual-diff/main.cpp:37` — `constexpr int kRows = 18`; `qt/tools/visual-diff/main.cpp:87` — `Qt::IgnoreAspectRatio`; `docs/adr/0021-visual-regression-gate-and-frame-pacing.md:26` — “the same change passed the 74 entries held to approved renders”.
  - Why: The comparator reduces images to coarse grids, normalizes contrast and searches positional shifts. Those tolerances help compare an implementation with a differently rendered artboard, but also forgive changes against an approved render. The ADR already records a 12-to-16 font-size regression passing all 74 approved-render entries. The large combined canary does not establish sensitivity to smaller colour, typography or spacing regressions.
  - Change: Remove artboard-comparison tolerances from comparisons against approved renders. Use a dimension-preserving, colour-sensitive comparison there; retain artboard scores as advisory evidence. Calibrate with separate, small regression plants.
  - Risk: Font and renderer variation can introduce noise. Pin the rendering environment and measure unchanged-run variance before choosing tolerances.

- **qa-packs/F8** (delete): The release contract step duplicates the CLI and omits REST verification
  - Where: `crates/dettivo-qa/src/pack/gate.rs:121` — `replay::run(&opts.repo_root, &daemon.socket)`; `crates/dettivo-qa/src/daemon_verbs.rs:211` — `report.rest = rest`.
  - Why: The ordinary contract command adds MCP and REST harness results. The release step starts another similarly configured daemon but calls only socket replay. MCP has a separate release step; REST does not. The checked-in strict-contract report consequently contains empty MCP and REST arrays despite its daemon enabling REST.
  - Change: Delete the second contract orchestration path. Share one library operation between the CLI and release pack, with explicit required transports and validated result sections. Avoid rerunning MCP twice.
  - Risk: Shared-daemon fixture ordering matters because fixtures mutate state. Verify the complete transport suite against a fresh profile and confirm a broken REST response fails the release step.

- **qa-packs/F10** (delete): The visual matrix has permanent gaps and a second Home implementation
  - Where: `qa/visual/manifest.toml:206` — `# Home joins here with the settings spec`; `qa/visual/manifest.toml:279` — `# The five sections that follow the pattern, General standing for them:`; `scripts/qa/app-visual-diff.sh:61` — `env QT_QPA_PLATFORM=offscreen QT_SCALE_FACTOR=1`.
  - Why: Home still uses a separate shell implementation with two themes and one scale. First run omits Tokyo Night. General stands in for four other settings sections, so their content-specific layout changes escape the matrix. The “every surface” claim exceeds the implemented coverage.
  - Change: Move Home into the manifest and delete its separate rendering, crop-loop and comparison orchestration. Enumerate the actual settings sections and restore the promised theme coverage.
  - Risk: Additional entries require reviewed baselines. Verify the inventory against real routes and plant a regression in one previously represented settings section.

- **qa-packs/F12** (simplify): The GPU proof establishes presence or aggregate activity, not workload execution
  - Where: `crates/dettivo-qa/src/pack/gpu_proof.rs:263` — `.find(|(_, n)| *n > 0)`; `crates/dettivo-qa/src/pack/gpu_proof.rs:274` — `let busy = with_engine`; `crates/dettivo-qa/src/pack/gpu_proof.rs:279` — `if busy >= AMD_BUSY_FLOOR`.
  - Why: NVIDIA passes after one process-table appearance, which establishes a GPU allocation but not where inference ran. AMD passes when any observed device reaches 10% activity while an engine process exists; the compositor or another application can supply that activity. The report's workload-proof claim is stronger than either observation.
  - Change: Delete the aggregate busy threshold as proof. Record these counters as diagnostics unless work can be attributed to the measured engine and device. Return unavailable proof when attribution is unsupported.
  - Risk: Some machines will lose a green proof row. Check an unrelated GPU load alongside CPU inference and an engine that initializes a GPU context but performs no inference there.

### qt-hosts

- **qt-hosts/F14** (simplify): Meeting import buffers and hashes the entire recording on the GUI thread
  - Where: `qt/host/app/meetings_actions.cpp:365`, `upload.bytes = file.readAll();`; line 411 hashes `upload.bytes` in one operation.
  - Why: The transfer protocol already supports chunks, but the host first retains the whole recording and later hashes it synchronously. Memory therefore scales with recording size, and both operations block window input and painting. Large files can exhaust memory before the daemon evaluates their size.
  - Change: Remove `Upload::bytes`. Stream bounded chunks from an open file, hash incrementally, and perform blocking media I/O outside the GUI thread.
  - Risk: Preserve chunk ordering and the final digest. Test a large recording, cancellation, read failure and a file changed during upload.

- **qt-hosts/F15** (simplify): The visual metric is too permissive to enforce approved-render fidelity
  - Where: `qt/tools/visual-diff/main.cpp:37`, `constexpr int kRows = 18;`; line 87 scales with `Qt::IgnoreAspectRatio`. `docs/adr/0021-visual-regression-gate-and-frame-pacing.md:26` records that a font-size increase “passed the 74 entries held to approved renders.”
  - Why: The comparator discards dimensions, most spatial detail and absolute colour information, then accepts shifted correlations. Those allowances help compare different artboards and fonts, but also normalize away regressions between a render and its approved counterpart. The record already demonstrates the resulting false passes.
  - Change: Use a stricter comparison for approved renders of the same theme and scale, with exact dimensions and explicit rasterization tolerance. Retain the permissive artboard score only as advisory evidence.
  - Risk: Pin the rendering environment before tightening tolerance. Check small independent regressions in colour, typography, spacing and missing controls, rather than relying only on the combined canary.

- **qt-hosts/F18** (simplify): The duplicated QA parser has a known-variable hole
  - Where: `qt/host/qa_environment.cpp:18` lists `"DETTIVO_E2E_DISCLOSURE"`; line 225 enforces QA mode only when `firstHook` was populated.
  - Why: Disclosure is recognized but never passed through `hook()` or validated. Qt therefore accepts it without QA mode and accepts invalid values that the Rust parser rejects. The larger structure duplicates daemon-specific fields and validation even though Qt does not consume most of them; the test mainly checks that 26 names appear in documentation.
  - Change: Separate recognition and QA-mode enforcement from parsing host-consumed values. Remove unused stored fields and use shared conformance cases for the Rust and Qt parsers, including every recognized hook.
  - Risk: Preserve inherited daemon hooks and typo rejection. Test each hook without QA mode, in a release build, and with an invalid value.

- **qt-hosts/F19** (delete): Completed first run still runs provisioning reads and a hotkey polling timer
  - Where: `qt/host/app/first_run_model.cpp:85–90` unconditionally reads keys, snippets, devices, selection and models; line 106 starts polling when the step is `"keys"`. `qt/host/app/first_run_model.h:161` defaults that step to `"keys"`.
  - Why: `markComplete()` does not deactivate this work. A provisioned app still initializes the first-run data path and starts the polling timer, alongside Settings’ duplicate hotkey and model reads. Home startup also unconditionally launches Agents checks and Doctor in `qt/apps/dettivo-app/app_main.cpp:319–321`.
  - Change: Activate provisioning only while first run is required or explicitly reopened. Load diagnostics when requested. Reuse the existing model catalogue and hotkey facts instead of maintaining duplicate fetch-and-parse paths.
  - Risk: Explicitly reopening onboarding and reconnecting on an active settings page must still refresh correctly. Check startup calls for a provisioned Home session.

### qml

- **qml/F3** (simplify): Hand-built controls leave essential actions outside keyboard navigation
  - Where: `qt/qml/Dettivo/app/SidebarItem.qml:66` — `TapHandler { … root.activated() }`. `qt/qml/Dettivo/components/SegmentedControl.qml:76` — `Accessible.role: Accessible.PageTab`. `qt/qml/Dettivo/app/meetings/SourceRow.qml:35` — `TapHandler`.
  - Why: These are rectangles or items with pointer handlers and accessible labels, but no tab-focus or keyboard activation implementation. Segments additionally lack an accessible activation handler. Similar controls independently redraw selection, borders and hover throughout the surfaces. Declaring a role does not supply control behavior.
  - Change: Use styled buttons, radio buttons, check boxes and delegates underneath these compositions. Keep the surface-specific labels and layout; delete their duplicate interaction and state drawing.
  - Risk: Preserve the approved appearance and accessible names. Traverse onboarding, navigation, mode selection and meeting sources using only Tab, arrows, Space and Enter.

- **qml/F13** (simplify): The analysis rail duplicates the full tab and overflows with ordinary long results
  - Where: `qt/qml/Dettivo/app/meetings/AnalysisRail.qml:19` uses an unbounded `Column`; lines 54–72 render the complete summary, decisions and action items. `qt/qml/Dettivo/app/meetings/AnalysisView.qml:18` already provides a scrolling view of that content.
  - Why: The rail has neither scrolling nor a content bound above its bottom-anchored facts. Longer results overlap those facts and continue outside the window. Maintaining two complete presentations also duplicates state handling.
  - Change: Prefer a bounded summary and an action opening the full Analysis tab; delete the repeated full lists from the rail. This changes the approved composition and needs design approval. Preserving the composition requires a bounded scrolling region.
  - Risk: Keep analysis status and actions visible. Test long summaries, many action items and larger fonts.

- **qml/F17** (delete): Delete the import target choice that cannot be submitted
  - Where: `qt/qml/Dettivo/app/meetings/ImportDialog.qml:187` offers `["Dictation", "Meeting"]`. `qt/qml/Dettivo/app/meetings/ImportFooter.qml:47` permits submission only with `root.ready && root.meeting`.
  - Why: Choosing Dictation leads to a disabled Import button. Its explanation sends the user to History, whose implemented screen has no corresponding import action. The backend adapter for this dialog imports meetings.
  - Change: Delete the unsupported target selector and its `meeting` branches; present this as meeting import. A future dictation import needs a complete submission path.
  - Risk: Re-approve the simpler dialog and update its keyboard/accessibility expectations. Preserve the working meeting import options.

- **qml/F18** (simplify): History and Meetings carry the same search control twice
  - Where: `qt/qml/Dettivo/app/history/SearchField.qml:12` and `qt/qml/Dettivo/app/meetings/MeetingsSearchField.qml:12` begin the same property interface; both implement the same focus helpers, clear action, hit count and debounce timer.
  - Why: These 108-line components differ principally in their placeholder and accessible strings. Every keyboard, accessibility or layout fix currently needs two edits.
  - Change: Keep one shared search component with explicit placeholder, field name, clear-action name and container name properties. Delete the duplicate implementation.
  - Risk: Preserve distinct accessible names and independent query state. Run both search flows through typing, Enter, clear and focus restoration.

### ops-and-record

- **ops-and-record/F15** (delete): Delete the manually maintained Makefile mirror
  - Where: `Makefile:2` says “keep these targets in step”; line 206 hardcodes `build-package.sh bin dist`, while the package target accepts `DIST`.
  - Why: Drift already exists. `make --dry-run package-bin DIST=build/review-dist` stages into the requested directory and packages from `dist`. Several QA targets also omit the build prerequisites present in their Just counterparts.
  - Change: Remove the duplicate recipe implementation and require Just. If compatibility is necessary, retain only a forwarding wrapper.
  - Risk: Existing Make users must migrate. Update the documented entry points and verify the canonical recipes cover them.

- **ops-and-record/F18** (delete): Delete the prose blacklist from the build gate
  - Where: `scripts/check-docs.sh:178` selects the first nonblank line after H1; line 186 rejects `note*|warning*|caveat*|"before you"*|"this document"*`.
  - Why: ADRs are judged on their `Status:` line, not their explanation. Other pages can fail merely because a useful opening begins with “Notes”. This checks vocabulary while the invalid commands in F14 remain accepted.
  - Change: Remove this heuristic and its planted wording test. Keep structural links, contract pins, indexed records, and the generated CLI-tree comparison; judge prose during review.
  - Risk: Stylistic problems stop blocking builds. Preserve the substantive documentation checks.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F20, Delete the redundant dispatch inventory and duplicate parameter decoding: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** agent-surfaces/F10, Compositor dictation overrides configured mode and implements toggle outside the daemon: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** agent-surfaces/F12, Three transfer implementations buffer whole files and disagree about cleanup: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** agent-surfaces/F14, Doctor can report success when required probes failed: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** agent-surfaces/F15, Delete two copies of the Unix-socket client: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** engines/F9, Model replacement temporarily keeps both models resident: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** engines/F16, The transcription crate carries an unused language dependency: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** dictation/F4, Delete the second transcript archive outside history retention: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** dictation/F5, Delete synthesized undo until it can identify the inserted range: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** dictation/F13, Disabled transforms and post-processors still run through unconditional passes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** meetings/F3, Delete the redundant meeting-row copy from metadata: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** meetings/F10, Collapse the completion hooks into one ordered post-processing path: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** meetings/F15, Status polling unnecessarily loads the entire meeting archive: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** qa-rig/F18, Process cleanup depends on reaching the runner’s normal teardown: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** qa-rig/F19, Delete the first-run scenario’s second model-store lifecycle: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** qa-packs/F6, The visual comparator deliberately ignores regressions the gate promises to catch: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** qa-packs/F8, The release contract step duplicates the CLI and omits REST verification: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** qa-packs/F10, The visual matrix has permanent gaps and a second Home implementation: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R19:** qa-packs/F12, The GPU proof establishes presence or aggregate activity, not workload execution: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R20:** qt-hosts/F14, Meeting import buffers and hashes the entire recording on the GUI thread: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R21:** qt-hosts/F15, The visual metric is too permissive to enforce approved-render fidelity: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R22:** qt-hosts/F18, The duplicated QA parser has a known-variable hole: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R23:** qt-hosts/F19, Completed first run still runs provisioning reads and a hotkey polling timer: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R24:** qml/F3, Hand-built controls leave essential actions outside keyboard navigation: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R25:** qml/F13, The analysis rail duplicates the full tab and overflows with ordinary long results: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R26:** qml/F17, Delete the import target choice that cannot be submitted: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R27:** qml/F18, History and Meetings carry the same search control twice: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R28:** ops-and-record/F15, Delete the manually maintained Makefile mirror: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R29:** ops-and-record/F18, Delete the prose blacklist from the build gate: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R30:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0054 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
