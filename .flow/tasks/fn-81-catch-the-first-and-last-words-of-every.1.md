---
satisfies: [R1, R2, R3, R4, R4a, R5, R6, R7]
---
# fn-81-catch-the-first-and-last-words-of-every.1 Implement Catch the first and last words of every take

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Every dictation take now records 200 ms after the release, so a word still sounding at the release is kept. A press released within 300 ms is a tap: it reaches no engine, plays no stop cue, stores nothing and ends `cancelled` with reason `tap`. With `[audio] keep_microphone_ready = true` (default false), the daemon keeps one capture open into a 10 s in-memory ring, and a take starts 450 ms before its press. The ring closes inside the capture reservation before a meeting opens its streams, reopens once the meeting's capture has ended, closes on a logind lock, follows device and configuration changes, and `audio.devices.microphone_ready` and `dettivo doctor` report its state (ADR 0092, ADR 0093).

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran (codex gpt-6.1-sol, 3-lens panel NEEDS_WORK with 5 findings, all fixed in a24aafd, one-reviewer re-review SHIP)

Decisions recorded (spec wording vs what shipped):
- The press time is the arrival instant fn-82 already passes (`dictation.press`/`release`/`start`/`stop`/`toggle`, portal, evdev). No `--pressed-at` flag was added (ADR 0093).
- The ring reopens when the meeting's capture has ended, after its worker stopped both tracks, not when the meeting enters `stopping`, because the meeting's streams stay open through `stopping` (ADR 0092). This keeps R4a's single capture.
- The daemon has no dictation-disable switch and no pause verb. "Disable" is turning the setting off. "Pause" has nothing to attach to and is recorded in ADR 0092. "Lock" is logind `LockedHint` or `Active = false`. A locker that does not set `LockedHint` (Omarchy's Quickshell lock, docs/qa.md) leaves the ring open while locked.
- While a pinned `input_device` is absent, the ring falls back to the default source (R6), and it returns when the device reappears.
- `Event::Pcm` now carries its capture stamp (`at`). The worker cuts the take at release + tail by capture time and writes the take once. Meeting code only changed its match pattern.
- The fn-65 rules (the meeting tap, the take-window span to silence, taps marking no span) are recorded in ADR 0092 and not built.

Tests for the ACs: crates/dettivo-session/tests/take_window.rs covers R1 (first word, 100 ms before the press), R2 (last word, release 150 ms early), the queued release, R3 (taps through stop and discard) and R5 (only the window is written). crates/dettivod/src/ready_tests.rs covers the R4a handoff with a capture counter, disable and lock, device moves, the pinned fallback, failure reporting and the ring staying open through transcription. crates/dettivod/tests/take_window.rs covers the daemon end to end: hold H gives a take of H+600 to H+720 ms, a tap leaves no stop cue and no history, and the meeting handoff shows the expected log order. crates/dettivo-cli/src/doctor_report.rs covers the R6 doctor row.

Follow-ups (not built):
- A daemon-level test of the refused-start tap release (finding E). The socket path cannot reach a refusal inside `Session::start`, so the fix is covered by inspection only.
- A lock signal for lockers that do not set `LockedHint`.
- No live PipeWire run: no real microphone was opened, as the run forbids. The single-capture guarantee rests on the audio thread processing commands in order, plus the counter and log-order tests.

Feature routes: Settings / General / Microphone gains "Keep microphone ready". `dettivo doctor` gains a `microphone ready` row.

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 7de3c60a8cb923e8845a6a3ddcc8a21511447437, a24aafd54b4da6d2d1d26ce90eb6af7ce546ea53
- Tests: cargo test -p dettivo-audio -p dettivo-session (rc 0, after a24aafd), cargo test -p dettivod --no-fail-fast (full, rc 0 at 7de3c60); after a24aafd: --bin dettivod + take_window hold hotkeys dictation dictation_gate dictation_mode contract config_live session_exclusion meetings smoke (rc 0), cargo test -p dettivo-meeting -p dettivo-cli (rc 0), cargo clippy --workspace --all-targets -D warnings (rc 0); cargo fmt --all --check (rc 0), just lint (all steps green at 7de3c60; docs/settings-keys re-run after a24aafd ok), baseline: green (cargo test -p dettivo-audio -p dettivo-session pre-edit)
- PRs: