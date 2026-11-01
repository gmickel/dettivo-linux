---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-85-delivery-that-leaves-the-desktop-as-it.1 Implement Delivery that leaves the desktop as it found it

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
All five delivery behaviours of fn-85 are in, plus QA P2 F3 and the QA P1 F1 fix under Gordon's R5 decision:
- R1: the clipboard snapshot restores every type, and only while Dettivo's text is still the clipboard. X11 now runs on x11rb, and x11-clipboard is dropped.
- R2: the first key waits a fixed 250 ms after the stop. This is Gordon's option (a), recorded in ADR 0090.
- R3: a quiet second dictation into the same window joins the first, using the ext-idle-notify v2 input signal.
- R4: `dettivo dictation stop --print` prints the text, records `printed` and exits 6 on no speech. F3: a plain `stop` on a silent take also answers CONFLICT noSpeech and exits 6.
- R5: `[audio] while_dictating` pauses, ducks or mutes media past the 300 ms tap. F1 (QA): duck steps are relative moves, and every ramp step, duck and restore, moves at most 0.04; the step count follows from the distance (23 smoothstep steps, about 0.75 s, for 0.80 to 0.20). A detected mid-ramp user change stops the ramp, is never restored over, and ends within 0.04 of the user's volume. A set landing inside one wpctl call can still be lost: the accepted residual, since PipeWire has no compare-and-set (Gordon, 2026-11-01, goal fn-88 log).
- R6: docs, ADRs 0090 and 0091 (now with R5's accepted meaning), linux-deltas and CHANGELOG.

Evidence (measured):
- cargo test -p dettivo-insert -p dettivo-session -p dettivo-cli: 170 passed.
- cargo test -p dettivod --bin dettivod -- media: 7 passed, including the 0.04 cap for the default duck and restore and the within-0.04 read-to-set race.
- R5 race sweep (r5race-rel.sh, private null sink, wpctl guard shim, 18 offsets 0 to 500 ms after the first step, two passes): 36 of 36 within 0.04 of the user's 0.66 (27 exact, 9 off by 0.02 to 0.04); no in-call loss observed.
- just lint green after the F1 fix; 86/86 Qt ctest earlier.
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
- R5 means bounded interference: at most 0.04 on a detected change; the in-call wpctl window is the accepted residual (Gordon).
- The restore stays synchronous inside the insertion, which is what keeps the first original on back-to-back dictations.
- The join window is a 30 s constant, with no config key.

stage: impl-review - ran [codex gpt-6.1-sol high: 3 rounds to SHIP at 0620f5a; QA F1 round NEEDS_WORK at 8c98453 -> re-review SHIP at 7ad6bd2]
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: eca1530b8f604b6f6df026032d39a3100564d9b4, ee07c4d0cdaf945ffb4f3a7124af9d9a1391b82f, f07b50ff5281d273f8afc1968f03a7badb4a724c, 9578e4597b083af29fbd825540b8c10b1086515e, b2a04441af7a461b533a2f1c019a0822f684825f, dfd4ea966d6c79d2c350979651dc5c6d6a1c5f07, 0620f5a60c5ae40e5f31ad5038cc1de382e1d086, fd1523902da297f2493f16d4842c35d2a51539e5, 8c98453ff7d2e84373e40e2c4e97c57915789e22, 7ad6bd26f7134f88fb7244b312c5753b74253252
- Tests: cargo test -p dettivo-insert -p dettivo-session -p dettivo-cli, cargo test --workspace (1 failure: stale evidence-map ref, fixed; cargo test -p dettivo-qa --lib evidence_map green), ctest --test-dir build/qt (86/86), cargo test -p dettivod --bin dettivod -- media (7 passed after F1 fix), just lint (green after F1 fix), scripts/qa/xvfb-session.sh .flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/tools/r5race-rel.sh 5 and 6 (private null sink: 36/36 within 0.04), scripts/qa/xvfb-session.sh X11 clipboard round trip (temporary test, removed)
- PRs: