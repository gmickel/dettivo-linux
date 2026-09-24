# Cleanup: delivery and the target window (12 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

Releasing a key delivers the intended text to the originating window or delivers nothing, and a cancel prevents delivery. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0047 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### dictation

- **dictation/F2** (contract): The captured target identifies an application process, not the originating window
  - Where: `crates/dettivod/src/actions.rs:53` — `Some(SessionTarget { app_id: Some(target.app_id), pid: target.pid, })`; `crates/dettivo-insert/src/chain.rs:37` — `if guards.app_id.is_none() && guards.pid.is_none() { return Ok(()); }`.
  - Why: Two windows belonging to the same process satisfy both guards. A failed focus probe at recording start also becomes an absent target, which means no origin guard; if probing works later, automatic insertion can proceed into a different window.
  - Change: Preserve a stable native window identifier internally. Represent “origin could not be verified” separately from an intentionally unguarded request, and route an unverified dictation origin to clipboard recovery.
  - Risk: Window identity differs between compositors and X11. Test two windows from one process, a closed origin, and a failed initial probe followed by a successful final probe.

- **dictation/F3** (bug): Backend failure blindly retries text that may already have been partially delivered
  - Where: `crates/dettivo-insert/src/service.rs:353` — `Err(e)` logs `"backend failed, trying the next"`; `crates/dettivo-insert/src/backend/commands.rs:15` — `Duration::from_secs(20)`; `crates/dettivo-insert/src/backend/commands.rs:155` — `inter_key_delay_ms.max(REMAP_DELAY_MS)`.
  - Why: Backend errors do not distinguish failure before delivery from partial or uncertain delivery. For example, non-ASCII text gives xdotool a minimum 40 ms delay; a sufficiently long transcript can reach the 20-second timeout after typing a prefix. The next backend receives the entire transcript again.
  - Change: Return structured delivery status: nothing delivered, partial delivery, or uncertain delivery. Automatically try another backend only when nothing was delivered; otherwise retain the transcript and report the incomplete result.
  - Risk: Some backends cannot determine exactly what the application received. Treat uncertainty conservatively and test failure after a delivered prefix, including command timeout.

- **dictation/F6** (bug): Focus and self-target guards expire before the backend actually types
  - Where: `crates/dettivo-insert/src/service.rs:251` — `let probed = self.probe(&session, settings)`; `crates/dettivo-insert/src/service.rs:281` — `self.candidates(&backends, &session)`; `crates/dettivo-insert/src/service.rs:315` — `backend.insert(request.text, &ctx)`.
  - Why: Guard checks happen before backend availability probing and setup. Portal interaction and multi-character typing can take appreciable time, during which focus may change—even to Dettivo itself. The eventual insertion still uses the earlier decision.
  - Change: Separate backend preparation from delivery. Revalidate the origin and self-target guard immediately before delivery, serialize insertion operations, and stop subsequent typing batches when focus changes.
  - Risk: Synthetic input cannot offer universal atomic focus guarantees. Test focus changes during portal setup and between typing batches, and report partial delivery honestly.

- **dictation/F7** (contract): The secure-field guard is unreachable in the real probe path
  - Where: `crates/dettivo-insert/src/service.rs:91` — `probe::target_from(&r, settings, false)`; `crates/dettivo-insert/src/service.rs:275` — `if target.is_some_and(|t| t.secure_field)`.
  - Why: Production probing always supplies `false` for secure-field status. The later clipboard-only branch therefore does not protect password fields, despite the insertion record describing that protection.
  - Change: Obtain focused-field sensitivity from an actual accessibility probe and represent unknown status explicitly. Align the documented guarantee with the detection that is implemented.
  - Risk: Accessibility information is incomplete in some applications. Verify ordinary text, a native password field and an inaccessible field through the real probe, rather than constructing a target with the bit already set.

- **dictation/F10** (bug): The primary compositor hotkey path ignores the configured dictation mode
  - Where: `crates/dettivo-cli/src/commands.rs:269` — `"mode": mode.clone().unwrap_or_else(|| "raw".into())`; `crates/dettivo-cli/src/commands.rs:283` — `"mode": "raw"`.
  - Why: Compositor snippets invoke the CLI’s start and toggle commands without a mode. Both paths explicitly send Raw, overriding the daemon’s configured default. Portal and evdev starts instead pass no override, so identical hotkeys select different processing behavior depending on backend.
  - Change: Make an omitted mode consistently mean the daemon’s configured mode. Preserve an explicit Raw override and handle the imported wire contract compatibly.
  - Risk: Existing clients may depend on required wire fields. Test compositor CLI, portal and evdev entry points with an Enhanced default and with an explicit Raw request.

### meetings

- **meetings/F9** (contract): The first live windows and final transcript do not share a verified sample clock
  - Where: `crates/dettivo-meeting/src/machine.rs:157` — microphone opening precedes system opening; `crates/dettivo-audio/src/takes.rs:108` — `let elapsed = self.started.elapsed()` stamps writer creation; `crates/dettivo-meeting/src/live.rs:129` — `Windower::new(source, settings.clone(), 0)`.
  - Why: Capture sources open before their writers, and the recorded offsets describe writer creation rather than the first captured samples. Both live lanes nevertheless start at zero, while finalisation applies the stored offsets. Checking that two writers started close together does not establish that their audio samples align.
  - Change: Carry a first-sample origin through capture into the take manifest and live windower. Use that same origin for finalisation.
  - Risk: This affects interleaving, gaps and speaker assignment. Delay one source’s startup and inject a shared audible landmark; measure the landmark alignment in live and final output.

### qa-rig

- **qa-rig/F5** (bug): The Hyprland hotkey scenario does not own the bindings it removes
  - Where: `crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs:27`: “The chords: Ctrl+Alt+Shift with a function key sits on no default binding”; `crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs:226`: `let _ = hyprctl(&["eval", UNBIND]);`.
  - Why: Absence from default bindings does not establish absence from Gordon’s bindings. The scenario installs fixed F9–F12 chords, then unconditionally unbinds them and ignores cleanup failures.
  - Change: Inspect existing bindings and reserve unused chords, or run in a dedicated compositor session. Track acquired bindings and make failed restoration a scenario failure.
  - Risk: Binding serialization may vary with Hyprland. Test a preexisting matching chord and a failed drive; the original binding must survive both.

- **qa-rig/F9** (test): First-run insertion failures are repaired before the scenario judges success
  - Where: `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:302`: `if outcome != "inserted" && reason.starts_with("CONFLICT")`; line 325 assigns `outcome = "inserted".into()`; `crates/dettivo-qa/src/scenarios/first_run_fresh.rs:398`: `report.insert("allowance_after_done".into(), allowance["reason"].clone());`.
  - Why: Failed automatic insertion triggers refocusing and a separate `insert.perform` call with `allow_self_target: true`. Success replaces the original outcome and bypasses the normal backend assertion. The post-Done allowance probe merely records its response without asserting refusal.
  - Change: Require the original insertion to succeed in the happy-path scenario. Move recovery into a separate scenario. Assert the expected post-Done refusal and absence of an insertion side effect.
  - Risk: Shared-desktop focus changes will become visible failures. Use controlled focus and preserve the original failure evidence rather than silently repairing it.

### qa-packs

- **qa-packs/F11** (bug): First-insert benchmarks discard failures and accept one successful warm sample
  - Where: `crates/dettivo-qa/src/bench/first_insert.rs:152` — `Err(e) => failures.push(e)`; `crates/dettivo-qa/src/bench/first_insert.rs:164` — `.filter(|r| !r.cold && r.outcome == "inserted")`; `crates/dettivo-qa/src/bench/first_insert.rs:186` — `if row.warm.is_none()`.
  - Why: Nine failed warm dictations and one successful dictation produce a measured row with p50 and p95 from the survivor. The failures remain in the JSON but do not fail the step. This biases latency toward successful runs and contradicts the intended ten-sample measurement.
  - Change: Require the configured number of successful warm runs. Preserve every failure and report incomplete sampling as failure, with partial statistics clearly separated from a qualifying measurement.
  - Risk: Flaky runs will stop producing apparently valid headline figures. Test one-of-ten, nine-of-ten and ten-of-ten completion.

### qml

- **qml/F9** (bug): The permitted minimum window size makes routes geometrically impossible
  - Where: `qt/qml/Dettivo/app/AppWindow.qml:58` — `minimumWidth: Theme.sidebarWidth * 3`. `qt/qml/Dettivo/app/history/HistoryRoute.qml:96` fixes the list width, while lines 120–122 anchor the detail into the remaining space.
  - Why: With default tokens, the window permits 600 px. The sidebar consumes 200 and History’s list consumes 400, leaving negative space after the separator for the detail. Settings and meetings also retain fixed columns without a narrow-window arrangement.
  - Change: Enforce a workable minimum derived from the visible columns now. Any supported smaller layout needs explicit collapsing or stacking, rather than shrinking the existing arrangement.
  - Risk: A minimum larger than the available desktop is also unusable. Check the smallest supported display and window size, larger theme fonts, and both scale factors.

- **qml/F10** (bug): Keyboard rename targets a different speaker from the selected transcript row
  - Where: `qt/qml/Dettivo/app/meetings/SpeakerTranscript.qml:25` changes only `focusRow`. `qt/qml/Dettivo/app/meetings/MeetingDetail.qml:43` renames `speakerAt(root.speakerIndex)`.
  - Why: Moving with `j` and `k` updates the highlighted transcript row but never updates `speakerIndex`. Pressing `r` therefore renames the initial speaker or the last speaker clicked, rather than the speaker on the selected row. The supplied `currentSpeakerId` property is not used by the transcript component.
  - Change: Derive the rename target from the selected segment’s speaker ID. Delete the disconnected selection state or synchronize it through one explicit selection signal.
  - Risk: Test alternating speakers, unlabeled segments and a transcript updated after diarization. Confirm the rename request contains the selected row’s speaker ID.

- **qml/F14** (bug): First run calls unsuccessful insertion “Inserted”
  - Where: `qt/qml/Dettivo/app/TryItStep.qml:31` — `return root.known ? qsTr("Inserted") : qsTr("Idle");`. `qt/host/app/first_run_model.cpp:313` sets `m_resultKnown = true` for all insertion outcomes. `TryItStep.qml:225` also asserts that the words are on the clipboard whenever a reason exists.
  - Why: “A result exists” is not “insertion succeeded.” Clipboard fallback and unsuccessful insertion receive the success label, and a reason alone does not prove clipboard success.
  - Change: Expose the actual outcome and derive the label, explanation and clipboard claim from it.
  - Risk: Exercise inserted, copied-to-clipboard and failed outcomes. Each should show consistent status text and recovery instructions.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** dictation/F2, The captured target identifies an application process, not the originating window: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** dictation/F3, Backend failure blindly retries text that may already have been partially delivered: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** dictation/F6, Focus and self-target guards expire before the backend actually types: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** dictation/F7, The secure-field guard is unreachable in the real probe path: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** dictation/F10, The primary compositor hotkey path ignores the configured dictation mode: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** meetings/F9, The first live windows and final transcript do not share a verified sample clock: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** qa-rig/F5, The Hyprland hotkey scenario does not own the bindings it removes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** qa-rig/F9, First-run insertion failures are repaired before the scenario judges success: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** qa-packs/F11, First-insert benchmarks discard failures and accept one successful warm sample: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** qml/F9, The permitted minimum window size makes routes geometrically impossible: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** qml/F10, Keyboard rename targets a different speaker from the selected transcript row: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** qml/F14, First run calls unsuccessful insertion “Inserted”: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0047 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
