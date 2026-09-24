# Cleanup: qa isolation and honest assertions (19 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

Every scenario runs in an isolated profile and its assertions accept only the behaviour the spec asked for. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0052 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F12** (test): Strict replay can succeed without exercising implemented stateful methods
  - Where: `crates/dettivo-qa/src/replay.rs:324`: `STATEFUL.iter().find(...)`, producing `Verdict::Skip`; line 329: `"covered by the daemon's tests"`; `crates/dettivo-qa/src/stateful.rs:90`: `"meetings/notes.set.json"`, excluded because `"updated_at is the daemon's clock"`.
  - Why: Strict success rejects failures and certain pending rows, but accepts these skips. The replacement coverage is a string assertion rather than an executed link to a test. Several exclusions concern ordinary state setup, generated identifiers or timestamps, so removing their implementation would not make strict replay catch them.
  - Change: Turn these fixtures into short scenarios with setup requests and captured identifiers, or require machine-checkable evidence from their corresponding integration tests. Preserve explicitly deferred methods admitted by false capability flags.
  - Risk: Keep model-dependent tests isolated from real model storage. Verify that disabling an advertised stateful handler makes the combined contract gate fail.

### agent-surfaces

- **agent-surfaces/F17** (test): `mcp check` reports framing support without exercising MCP
  - Where: `crates/dettivo-cli/src/mcp.rs:162` — calls `system.version`; `crates/dettivo-cli/src/mcp.rs:179` — reports framing names from enum constants.
  - Why: The command queries the daemon and counts locally constructed definitions. It never launches or initializes an MCP server and never sends a framed message, despite being described as an initialization/framing check.
  - Change: Run a bounded subprocess handshake through `mcp serve`, followed by `tools/list` and a harmless daemon-backed call. Report which transport checks actually ran.
  - Risk: Test server startup failure, stdout contamination, malformed framing and handshake timeout. Ensure the check always cleans up its child.

### dictation

- **dictation/F14** (test): The golden tests bypass the stage ordering used by dictation
  - Where: `crates/dettivo-language/tests/goldens.rs:91` — `rewrite(&case.input, ...)`; `crates/dettivo-language/tests/goldens.rs:120` — `apply_post_processors(&processors, &case.input)`; `crates/dettivo-language/src/raw/mod.rs:28` — `("colon", ":")`, and line 34 — `("dash", " - ")`.
  - Why: The goldens supply original phrases directly to later stages. Real dictation runs Raw first. For example, the corpus expects spoken “dash dash model qwen three colon 4b” to become technical syntax, but Raw consumes “dash” and “colon” before the later phrase recognizers see them.
  - Change: Keep the unit goldens and add acceptance cases through `pipeline::run` with explicit mode, application context and Raw settings. Fix the stage ownership of spoken technical syntax based on those results.
  - Risk: Unit parity and useful end-to-end behavior may disagree. Assert the actual inserted string for representative prose, command, filename and model-name cases.

### qa-rig

- **qa-rig/F7** (bug): Audio-rig cleanup can unload another scenario’s sink
  - Where: `scripts/qa/audio-rig.sh:25`: `pactl load-module ... >/dev/null`; `scripts/qa/audio-rig.sh:47`: `$2 == "module-null-sink" && index($0, s)`.
  - Why: Creation discards the module ID. Cleanup reconstructs ownership through substring matching, so requesting `dettivo-qa-mic-123` also matches `dettivo-qa-mic-1234`. Creation also leaves the module loaded if its monitor never appears.
  - Change: Return and retain the module ID from creation, unload that exact module, and roll back failed setup.
  - Risk: Callers currently expect a monitor name. Update the small caller set together; test overlapping names and monitor-discovery failure with a stub `pactl`.

- **qa-rig/F10** (bug): Settings comparison accepts unequal floating-point values
  - Where: `crates/dettivo-qa/src/scenarios/settings_support.rs:157`: `typed.parse::<f64>().ok() == n.as_f64() || typed.parse::<i64>().ok() == n.as_i64()`.
  - Why: For typed `0.35` and returned `0.9`, the float comparison fails but both integer conversions yield `None`. `None == None` makes the entire predicate true. Invalid numeric text can pass for the same reason.
  - Change: Compare only successfully parsed numeric values. Parse expected tables into complete objects as well, so surplus returned entries cannot pass a round-trip assertion.
  - Risk: Preserve intentional daemon coercions. Add negative cases for unequal floats, invalid numbers, and unexpected table entries.

- **qa-rig/F11** (test): History validation substitutes row counts and unknown geometry for correct contents
  - Where: `crates/dettivo-qa/src/scenarios/history_seed.rs:221`: `is_full(&tree).unwrap_or(true)`; line 223: `if shown == expected || (folded && joined)`.
  - Why: The exact-count branch ignores the computed row-content and insertion checks. When fewer rows appear, unknown list geometry is treated as proof that the rest are below the fold. Wrong rows at the right count, or a truncated tree, can pass.
  - Change: Check expected row identities and order in every branch. For virtualized lists, scroll and verify the required contents; report incomplete evidence when viewport capacity is unknown.
  - Risk: Accessibility tokens may change between snapshots. Use domain row identity, and test reordered rows, equal-count wrong rows, and a truncated snapshot without bounds.

- **qa-rig/F12** (test): A failed history rerun counts as a settled successful rerun
  - Where: `crates/dettivo-qa/src/scenarios/history_seeded.rs:337`: `if item["status"] != "transcribing" { return Ok(got); }`.
  - Why: Anything other than `transcribing`, including failure or a missing status, satisfies the predicate. The parent link is checked, but successful transcription is not.
  - Change: Wait for the explicit successful terminal status and assert the fixture transcript. Fail immediately on a failed terminal status, retaining its diagnostic.
  - Risk: Valid intermediate states must remain waitable. Exercise successful inference, engine failure, and malformed status responses.

- **qa-rig/F13** (test): The meeting device-switch check counts audio recorded before the switch
  - Where: `crates/dettivo-qa/src/scenarios/meeting_rig.rs:271`: `sys.len().saturating_sub(16_000)`; `crates/dettivo-qa/src/scenarios/meeting_rig.rs:336`: `if system_samples_after_switch < 8_000`.
  - Why: Recording begins before a 400 ms delay, a one-second fixture, and another 300 ms delay. Subtracting only one second counts pre-switch samples as post-switch continuity. The system track can stop at the device switch and still exceed the threshold; its correlation check examines the early recording.
  - Change: Record the actual switch boundary and verify the second system fixture after that boundary.
  - Risk: Account for capture buffering explicitly. A regression fixture that truncates the system track at the switch must fail.

- **qa-rig/F14** (test): The live-transcription scenario never proves delivery while recording
  - Where: `crates/dettivo-qa/src/scenarios/meeting_live.rs:192`: `sleep(Duration::from_millis(12_500))`; line 195 calls `meetings.stop`; line 199 begins `events.collect`.
  - Why: Notifications are consumed only after stop and finalization. Checking provisional/final ordering in the collected sequence does not prove that provisional text reached a consumer while the meeting was running.
  - Change: Collect concurrently with monotonic receive timestamps. Require provisional delivery for each source before issuing stop, then verify finalization.
  - Risk: Avoid a machine-specific latency threshold unless the product promises one. A daemon stub that releases all notifications after stop must fail.

- **qa-rig/F15** (contract): The cua text parser truncates labels, and scenarios accommodate the loss
  - Where: `crates/dettivo-qa/src/driver/cua.rs:408`: `after_quote.find('"')`; `crates/dettivo-qa/src/scenarios/history_states.rs:224`: `if !named && longer`.
  - Why: Markdown parsing ends a name at the first quote without decoding quoting or multiline text. The history scenario explicitly accepts only `No match for ` when the driver loses the query. The two drivers consequently enforce different product behavior.
  - Change: Obtain complete structured text or correctly decode the pinned upstream representation. Delete prefix-only accommodations and run shared snapshot/value conformance cases against both drivers.
  - Risk: Upstream output formats may change. Cover embedded quotes, escapes, multiline labels, and values on named controls.

- **qa-rig/F16** (test): Negative-text scanning misses values and exempts entire mixed-content strings
  - Where: `crates/dettivo-qa/src/negative_text.rs:98`: `classify(&e.name)`; line 111: `f.name.contains(PROFILE_ROOT_PREFIX)`.
  - Why: Only accessible names are scanned, although text may live in `Element.value`. A raw-path finding is entirely discarded when its string contains `/tmp/dq`, even if the same string also exposes another path or developer diagnostic.
  - Change: Scan relevant product-owned names and values. Normalize only the exact current profile-path span, then classify the remaining text. Share that policy across scenario helpers.
  - Risk: User transcripts can legitimately contain technical text. Define that boundary and test mixed profile/real paths plus diagnostics in named text fields.

- **qa-rig/F20** (test): The Omarchy-bar visual check can pass without the widget returning to idle
  - Where: `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:82`: “The right third of the bar”; `crates/dettivo-qa/src/scenarios/omarchy_bar.rs:217`: `if restored.score < changed.score`.
  - Why: Any change in a large crop can satisfy the listening check, including unrelated bar widgets. Equal listening and after scores pass restoration, so a Dettivo indicator stuck in its active appearance can pass.
  - Change: Capture the Dettivo widget specifically and compare explicit idle/listening states. Require restoration against an idle criterion, rather than merely being no worse than the listening comparison.
  - Risk: Theme rendering can vary. Test a frozen-active widget and an unchanged Dettivo widget beside an updating clock; both must fail.

### qt-hosts

- **qt-hosts/F20** (test): Host tests validate invented JSON and synchronous replies instead of the socket contract
  - Where: `qt/host/app/fake_link.h:24–25`, `reply(answers.value(method), {});`; `qt/host/app/first_run_model_test.cpp:93` invents a `"path"` result for `config.path`; `qt/host/app/app_meetings_test.cpp:366` explicitly expects `"provider_id"` in the selection request.
  - Why: The fake returns success by method name regardless of request shape and calls callbacks inline. It blesses F5’s rejected request and hides asynchronous save races. The invented config result also conceals a real broken action: `qt/host/app/first_run_model.cpp:415` reads `"path"` when the daemon returns `"config"`, so Open config cannot obtain the correct filename.
  - Change: Use the checked-in contract fixtures for wire shapes and fix that field lookup. Add controllable queued replies and a small local-socket integration suite covering framing, delayed replies, subscription and reconnect behavior. Keep lightweight fakes for formatting tests.
  - Risk: Avoid rebuilding the daemon inside a fake. Tests should reject malformed requests and exercise ordering explicitly, while the real contract suite remains authoritative.

### qml

- **qml/F4** (test): Application renders bypass the negative style check entirely
  - Where: `qt/apps/dettivo-app/app_main.cpp:389` — `const QImage image = window->grabWindow();`, followed by saving and quitting. `qt/host/render.cpp:38` — `styleFindings(window, fontFamily)`.
  - Why: The app implements a separate screenshot path that never calls the shared checker or plants its negative-test controls. Consequently, the app surfaces in the visual manifest can pass or receive approval without the style enforcement promised by ADR 0021.
  - Change: Delete the duplicate rendering path and use the shared render-and-check implementation. Register the planted-control and planted-font checks for the app.
  - Risk: Previously hidden violations will begin failing, including surface-owned control backgrounds. Fix those violations rather than exempting the app.

- **qml/F11** (test): The visual comparator cannot enforce the theme contract
  - Where: `qt/tools/visual-diff/main.cpp:37` — `constexpr int kRows = 18`; line 87 scales with `Qt::IgnoreAspectRatio`. `qa/visual/manifest.toml:22` sets `threshold = 0.55`.
  - Why: The comparator reduces pictures to coarse contrast and saturation grids, normalizes away information and tolerates shifts. ADR 0021 explicitly records that a colour-only regression passes and that a font-size increase passed the approved-render comparisons. That is inadequate for enforcing per-theme colour, typography and geometry.
  - Change: Retain the structural score for comparisons with HTML artboards. Compare approved QML renders against matching theme-and-scale renders using a stricter, colour-sensitive check that preserves dimensions.
  - Risk: Control font and rendering variation before tightening the gate. Use separate canaries for colour, typography, geometry and missing controls.

- **qml/F12** (test): The manifest does not cover every surface on five palettes at two scales
  - Where: `qa/visual/manifest.toml:153` lists only four first-run themes; line 206 still says `Home joins here with the settings spec`; lines 279–288 use General to represent five settings sections.
  - Why: The manifest expands to 492 entries but contains no Home surface. Its separate Home script checks only two themes at 1x. Several dialogs, non-General settings sections, and loading/error states have no corresponding manifest entries. A representative settings page cannot catch section-specific layout failures.
  - Change: Move Home into the manifest and delete its separate matrix runner. Restore Tokyo Night for first run and add the missing concrete routes and states, including export, re-run and confirmation dialogs.
  - Risk: New entries may expose missing approvals. Preserve first-approval failures and obtain actual approval rather than accepting generated references automatically.

### ops-and-record

- **ops-and-record/F10** (test): The session install check reports failed setup as success
  - Where: `scripts/packaging/install-test-session.sh:33` captures failure with `|| code=$?`; line 34 unconditionally records `session_setup_check pass 0`.
  - Why: Broken compositor setup receives a green receipt. Line 30 also selects Omarchy because the CLI advertises the command, which says nothing about the installed desktop.
  - Change: Judge the exit status and setup result. Detect the actual desktop or accept an explicit test target.
  - Risk: Plain Hyprland installations must remain supported. Test failed setup, missing snippet inclusion, Omarchy, and plain Hyprland.

- **ops-and-record/F17** (test): The installed-file check can miss the entire plugin
  - Where: `packaging/manifest.txt:28` lists `usr/share/dettivo/omarchy/`; `scripts/packaging/check-manifest.sh:62` removes subtree entries from comparison, and line 99 enters file-list comparison without checking subtree presence.
  - Why: Root-tree mode checks that the directory exists and contains a file. Installed-list mode discards all plugin paths and never requires any back, so a package missing the whole plugin can pass. Neither mode verifies its required entry points.
  - Change: Enumerate the small plugin file set, removing the subtree exception, or enforce identical required-entry checks in both modes.
  - Risk: Plugin file additions must update the manifest. Test an absent plugin, missing entry point, and unexpected packaged file.

- **ops-and-record/F19** (record): Accepted ADRs leave contradictory instructions without clear supersession
  - Where: `docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md:21` says packaging runs “on every push”; `docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md:18` assigns it to nightly regression. `docs/adr/README.md:3` requires the old record to receive “a pointer forward”.
  - Why: ADR 0034 remains simply Accepted without a supersession pointer. Similar historical promises survive in ADR 0004’s meeting-capable Parakeet introduction despite ADR 0018’s contrary outcome. Readers must reconstruct chronology to know which instructions govern.
  - Change: Mark affected portions superseded and link directly to the governing records. Keep historical evidence, but remove present-tense ambiguity from their introductions and index entries.
  - Risk: Historical decisions must remain recoverable. Check each changed status against the current implementation and preserve the original rationale.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F12, Strict replay can succeed without exercising implemented stateful methods: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** agent-surfaces/F17, `mcp check` reports framing support without exercising MCP: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** dictation/F14, The golden tests bypass the stage ordering used by dictation: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** qa-rig/F7, Audio-rig cleanup can unload another scenario’s sink: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** qa-rig/F10, Settings comparison accepts unequal floating-point values: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** qa-rig/F11, History validation substitutes row counts and unknown geometry for correct contents: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** qa-rig/F12, A failed history rerun counts as a settled successful rerun: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** qa-rig/F13, The meeting device-switch check counts audio recorded before the switch: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** qa-rig/F14, The live-transcription scenario never proves delivery while recording: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R10:** qa-rig/F15, The cua text parser truncates labels, and scenarios accommodate the loss: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R11:** qa-rig/F16, Negative-text scanning misses values and exempts entire mixed-content strings: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R12:** qa-rig/F20, The Omarchy-bar visual check can pass without the widget returning to idle: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R13:** qt-hosts/F20, Host tests validate invented JSON and synchronous replies instead of the socket contract: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R14:** qml/F4, Application renders bypass the negative style check entirely: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R15:** qml/F11, The visual comparator cannot enforce the theme contract: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R16:** qml/F12, The manifest does not cover every surface on five palettes at two scales: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R17:** ops-and-record/F10, The session install check reports failed setup as success: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R18:** ops-and-record/F17, The installed-file check can miss the entire plugin: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R19:** ops-and-record/F19, Accepted ADRs leave contradictory instructions without clear supersession: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R20:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0052 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
