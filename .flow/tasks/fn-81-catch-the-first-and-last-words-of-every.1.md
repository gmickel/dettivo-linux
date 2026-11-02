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

### Evidence
- Commits: 7de3c60a8cb923e8845a6a3ddcc8a21511447437, a24aafd54b4da6d2d1d26ce90eb6af7ce546ea53
- Tests: cargo test -p dettivo-audio -p dettivo-session (rc 0, after a24aafd), cargo test -p dettivod --no-fail-fast (full, rc 0 at 7de3c60); after a24aafd: --bin dettivod + take_window hold hotkeys dictation dictation_gate dictation_mode contract config_live session_exclusion meetings smoke (rc 0), cargo test -p dettivo-meeting -p dettivo-cli (rc 0), cargo clippy --workspace --all-targets -D warnings (rc 0); cargo fmt --all --check (rc 0), just lint (all steps green at 7de3c60; docs/settings-keys re-run after a24aafd ok), baseline: green (cargo test -p dettivo-audio -p dettivo-session pre-edit)
- PRs:

QA fix round (QA NEEDS_WORK at ae3e5e0, receipt qa-fn-81-catch-the-first-and-last-words-of-every.json; fixed 2190ae1..4703e08):
- QA81-1 (P1): the ring now closes on an Omarchy lock. dettivod follows Hyprland's hyprland_lock_notify_v1 (crates/dettivod/src/ready_screen.rs, protocol XML vendored), which Hyprland sends for every ext-session-lock locker including Omarchy's Quickshell lock, beside logind. No Omarchy config change or plugin hook is needed. audio.devices.microphone_ready.lock_signal reports compositor|logind|none and doctor warns when a lock may go unseen. A watcher lost while locked keeps the lock (review finding). Tests: ready_screen_tests.rs (fake compositor on a socket pair: lock, unlock, starts locked, lost while locked, no protocol), doctor_report lock-signal rows; live: isolated daemon + stand-in compositor + fake logind, S12a-f.
- QA81-4 (P1): capture reconnects after a PipeWire restart. The graph clears an alive flag on the core EPIPE, captures end with Error("PipeWire went away"), the audio thread reconnects every second, opens answer NoPipeWire meanwhile. Test: crates/dettivo-audio/tests/reconnect.rs (private PipeWire + WirePlumber, stop and restart; skips without them). Live: S10/S11 reruns pass.
- QA81-3 (P2): audio.devices.pipewire follows the live connection; the ring reports failed with the PipeWire reason and reopens on return; the doctor row names it without promising capture on press. Test: doctor_report row test; live S10a/S10c.
- QA81-2 (P2): dictation.pause / dictation.resume (dettivo dictation pause|resume): a runtime switch that closes the ring until resume or a daemon restart, no config write, a key press still dictates. Recorded in ADR 0092. Test: crates/dettivod/tests/take_window.rs pause_closes_the_ready_microphone_until_resume.
- Also fixed: capture streams are destroyed under the PipeWire loop lock (the "pw_stream_destroy called from wrong context" warning in the QA logs is gone).

Tier: implementer (dispatch: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran (codex gpt-6.1-sol, 3-lens panel NEEDS_WORK with 1 finding (lost watcher cleared a lock), fixed in 4703e08, one-reviewer re-review SHIP)

Follow-ups (not built):
- The graph registers only Audio/Source and Audio/Sink nodes, so a PipeWire "Audio/Source/Virtual" source cannot be pinned (QA S9b, pre-existing).
- tests/reconnect.rs and rig.rs skip in CI (no wireplumber); local runs cover them.

stage: plan-sync - skipped(config: planSync.enabled != true)

Pre-QA-recheck round (sibling fix, merge with the goal base, re-review; cb10c6b..70c65cd):
- Late PipeWire (sibling of QA81-4): the audio service no longer gives up when PipeWire is absent at daemon start. It starts its thread, tries PipeWire once, and retries every second as after a restart. Meanwhile audio.devices.pipewire is false, doctor reports capture unavailable (and `microphone ready failed: PipeWire is not reachable: PipeWire is not running` with the ring on), a press fails with that reason, and pinning a device skips node validation while disconnected. Test: crates/dettivo-audio/tests/reconnect.rs capture_follows_pipewire_starting_late_and_restarting (red at 725994d, green at 16ec48b; private PipeWire + WirePlumber). Live: .flow/tmp/fix81b/live/s13_late.sh, isolated daemon started with no PipeWire, private PipeWire started later, dictation transcribed without a restart, ring off and on (rerun on the merged build).
- Merged origin/goal/fn-88-better-dictation-shipped-as-one-release (8cd6c76: fn-80, fn-85). No overlap with dictation.pause/resume: fn-85's "pause" is the MPRIS while_dictating mode. Resolutions: finish code lives in crate::finish with Exit::Tap added; stop_expected and stop_print share stop_with and both take the release instant (print refusal checked before tap/tail); the source is not stopped at the stop request (release tail); record reports the stop instant that delivery's MODIFIER_SETTLE waits from; the continuation join gets the press time; feedback keeps Stopped (media) and Tapped (media back, no stop cue); silent takes end NoSpeech with the tail timing assertion kept; print_default.json carries keep_microphone_ready.
- Also fixed: the Qt settings sample lacked audio.keep_microphone_ready (78cc660; dettivo-settings-test failed before it), and a private rustdoc link (70c65cd).

stage: impl-review - ran (codex gpt-6.1-sol, 3-lens panel SHIP, no findings, head 70c65cd)

Tier: implementer (dispatch: claude-opus-5-5) (model: claude-opus-5-5)

QA recheck fix round (QA SHIP at 92af535 with one open P2, QA81-5; fixed in 3f20482):
- QA81-5 (P2, privacy): a daemon started while the compositor already reports locked no longer opens the ring for ~250 ms. The screen-lock watcher no longer reports "unlocked" on binding the notifier; it reports unlocked only after a second roundtrip delivered no initial `locked`. The compositor's lock slot starts pending (counts as locked) before the first reconcile (`MicReady::lock_pending`; `ready_screen::watch` now runs before `ready::supervise` starts), and a missing Wayland display or a failed watcher thread clears it so logind alone decides. Test: crates/dettivod/src/ready_screen_tests.rs a_session_locked_at_start_never_opens_the_ring (stand-in compositor sending `locked` at once and at the next roundtrip; red with the old watcher: reports [(false, false), (true, true)], green after). Live: QA scenario S9b (scratchpad h2/s9b_lock_edges.sh; private PipeWire, fake logind, stand-in compositor) clean in 4 of 4 runs: first compositor report locked=Some(true), no capture stream before the unlock (old run 4 had one 2.3 s before it). Logs: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r2/s9b-daemon-fix-{1..4}.log and s9b-pw-graph-fix-{1..4}.jsonl.

stage: impl-review - ran (codex gpt-6.1-sol high, 3-lens panel SHIP, no findings, base 7e725a3, head 3f20482)

QA81-5 sibling (logind source; fixed in d7a58ce):
- The logind lock source counted as unfollowed until its first read, so on a compositor without lock notifications the first reconcile could open the ring under a locked session. Its slot now starts pending (counts as locked) when `ready_session::watch` is called, before `ready::supervise` starts (main.rs order: screen watch, session watch, supervise). The first successful read of LockedHint and Active replaces it; an unreachable logind clears it, so with no compositor signal `lock_signal` reads `none` and the ring opens, as ADR 0092 documents. `ready_session::follow` takes its bus and ring so a test can drive it. Test: crates/dettivod/src/ready_session_tests.rs the_ring_waits_for_the_first_logind_read (stand-in logind on a private dbus-daemon: locked at once, locked with the first reply 400 ms late, unlocked 400 ms late, no logind -> `none` and not blocked); red without the pending mark (panics at "the ring opened before logind's first read"), green with it. Live: S9c (scratchpad h2/s9c_logind_locked.sh; no Wayland display, private PipeWire, stand-in logind starting LockedHint=true with its first reply held 1.5 s): without the fix the ring opened at start and captured dq81-mic for 1.7 s before the read; with it, 4 of 4 runs had no capture stream between start and unlock, lock_signal `logind` and closed at +0.3 s, open 0.18 s after unlock, closed on relock, and a daemon with no logind reported `none`/open with "logind unreachable" logged. Logs: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r2/s9c-daemon-{red,1..4}.log, s9c-pw-graph-{red,1..4}.jsonl.

stage: impl-review - ran (codex gpt-6.1-sol high, 3-lens panel SHIP, no findings, base 9168774, head d7a58ce)


Tier: implementer (dispatch: claude-opus-5-5) (model: claude-opus-5-5)
## Evidence
- Commits: 7de3c60a8cb923e8845a6a3ddcc8a21511447437, a24aafd54b4da6d2d1d26ce90eb6af7ce546ea53, 13a6502, e8fab41, 7662111, 2e462ce, b47cbae, 458469b, 849ac6e, 4703e08, 725994d, 16ec48b, 8cd6c76, 78cc660, 70c65cd, 3f20482, d7a58ce
- Tests: cargo test --no-fail-fast -p dettivo-audio -p dettivo-session -p dettivod -p dettivo-cli -p dettivo-qa -p dettivo-core -p dettivo-insert (rc 0, 665 passed, after merge 8cd6c76; WAYLAND_DISPLAY unset, PIPEWIRE_REMOTE and PULSE_SERVER at nonexistent sockets), cargo test -p dettivo-audio --test reconnect (private PipeWire + WirePlumber: red at 725994d, green at 16ec48b), ctest --test-dir build/qt (offscreen, 86/86 after 78cc660), just lint (rc 0 at 70c65cd), live: .flow/tmp/fix81b/live/s13_late.sh (late private PipeWire, ring off and on; pre- and post-merge), baseline: green (cargo test -p dettivo-audio --test reconnect at cb10c6b), earlier rounds: see the done summary, cargo test -p dettivod ready_screen (red with the old watcher, green at 3f20482), cargo test -p dettivod ready (rc 0, 10 passed) and take_window (rc 0) at 3f20482; PULSE_SERVER/PIPEWIRE_REMOTE isolated, no WAYLAND_DISPLAY, just lint (rc 0 at 3f20482), live: QA S9b x4 on the private harness at 3f20482, 0 of 4 opened the ring under the lock, cargo test -p dettivod ready_session (red without the logind pending mark, green at d7a58ce), cargo test -p dettivod ready (rc 0, 11 + 1 passed at d7a58ce; WAYLAND_DISPLAY unset, PIPEWIRE_REMOTE/PULSE_SERVER nonexistent), just lint (rc 0 at d7a58ce), live: S9c x4 on the private harness at d7a58ce (no compositor, logind starts locked, first reply +1.5 s): 0 of 4 opened the ring before the read; no-logind daemon reports lock_signal none; red run without the fix opened it for 1.7 s
- PRs: