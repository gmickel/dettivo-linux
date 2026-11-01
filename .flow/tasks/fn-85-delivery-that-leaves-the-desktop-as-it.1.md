---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-85-delivery-that-leaves-the-desktop-as-it.1 Implement Delivery that leaves the desktop as it found it

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
All five delivery behaviours of fn-85 are in, plus QA P2 F3:
- R1: the clipboard snapshot restores every type, and only while Dettivo's text is still the clipboard. X11 now runs on x11rb, and x11-clipboard is dropped.
- R2: the first key waits a fixed 250 ms after the stop. This is Gordon's option (a), recorded in ADR 0090.
- R3: a quiet second dictation into the same window joins the first, using the ext-idle-notify v2 input signal.
- R4: `dettivo dictation stop --print` prints the text, records `printed` and exits 6 on no speech. F3: a plain `stop` on a silent take also answers CONFLICT noSpeech and exits 6.
- R5: `[audio] while_dictating` pauses, ducks or mutes media past the 300 ms tap.
- R6: docs, ADRs 0090 and 0091, linux-deltas and CHANGELOG.

Evidence (measured):
- cargo test -p dettivo-insert -p dettivo-session -p dettivo-cli: 170 passed.
- The dettivod media, inserter and print-race tests pass.
- The full workspace run had 1 failure, a stale test name in qa/evidence-map.toml. It is fixed, and that test passes.
- 86/86 Qt ctest; clippy, fmt, the docs, deltas, file-length, settings-keys, qml and omarchy lints are all green.
- X11 clipboard round trip in private Xvfb :185: a 1.5 MB image from xclip, a two-type offer and a copy during the delay all passed. Whether xclip used INCR is not confirmed.

Open:
- The R1 and R2 live Hyprland QA drives and the R3 join on a live compositor were not run. Live-desktop QA is forbidden this run.
- R2's modifiers_held fallback is not built, because there is no key-state source (ADR 0090).
- Follow-up: measure how long the chord stays held after the stop, to tune the 250 ms.
- Settings now carries "Media while dictating" (Hotkeys) and "Join consecutive dictations" (Insertion).
- No feature-map route changed.

Decisions:
- The settle applies to every stop, not only toggles.
- The duck target is min(volume, 20 %).
- The restore stays synchronous inside the insertion, which is what keeps the first original on back-to-back dictations.
- The join window is a 30 s constant, with no config key.

stage: impl-review - ran [codex gpt-6.1-sol, 3 rounds: NEEDS_WORK (5 findings fixed) -> NEEDS_WORK (1 fixed) -> SHIP]
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: eca1530b8f604b6f6df026032d39a3100564d9b4, ee07c4d0cdaf945ffb4f3a7124af9d9a1391b82f, f07b50ff5281d273f8afc1968f03a7badb4a724c, 9578e4597b083af29fbd825540b8c10b1086515e, b2a04441af7a461b533a2f1c019a0822f684825f, dfd4ea966d6c79d2c350979651dc5c6d6a1c5f07, 0620f5a60c5ae40e5f31ad5038cc1de382e1d086
- Tests: cargo test -p dettivo-insert -p dettivo-session -p dettivo-cli, cargo test --workspace (1 failure: stale evidence-map ref, fixed; cargo test -p dettivo-qa --lib evidence_map green), ctest --test-dir build/qt (86/86), cargo test -p dettivod --bin dettivod -- media inserter, scripts/qa/xvfb-session.sh X11 clipboard round trip (temporary test, removed)
- PRs: