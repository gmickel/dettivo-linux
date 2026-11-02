---
title: Doctor says microphone ready open after PipeWire stops — every dictation silentl
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivod/src/ready.rs
tags: [qa, fn-81-catch-the-first-and-last-words-of-every, doctor]
problem_type: runtime-error
symptoms: "After PipeWire stops or restarts, doctor/audio.devices report microphone ready open while no capture exists; takes cancel as silence"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
After PipeWire stops (and also after it comes back), `dettivo doctor`, Settings / Diagnostics and `audio.devices` keep reporting `microphone ready open: takes start 450 ms before the press`, while no capture exists. Every dictation then ends `cancelled` with "silence: level". R6 asks that a ring failure shows in doctor rather than as a failed dictation; here it shows only as failed dictations and doctor says all is well.

## Steps to reproduce (cold)
1. Isolated profile daemon on a private PipeWire graph, `[audio] keep_microphone_ready = true`; ring `open`, one capture on the virtual microphone.
2. Stop the private PipeWire (pipewire + wireplumber), wait 3 s.
3. `dettivo doctor` -> `microphone ready open: takes start 450 ms before the press`; `audio.devices` -> `microphone_ready.state = "open"`, `pipewire: true`.
4. `dettivo dictation press`, 0.8 s, `release` -> `CONFLICT sessionCancelled`; log "take is silent; nothing sent to the engine ... dictation starts from the always-ready microphone".
5. Start PipeWire again, wait 6 s: ring still reported `open`, zero Dettivo captures in pw-dump, next dictation again cancelled as silent.
Reproduced twice (s4 run 1 and run 2).

## Expected
R6: "A default source change or device loss while idle reopens the ring on the new default without user action. A failure is reported in `dettivo doctor`, not as a failed dictation."

## Actual
The ring's state is driven only by the capture's `Ended` event; a PipeWire server that goes away sends none, so the ring stays `open` forever and its doctor row says takes start 450 ms early.

## Evidence
- console: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s4_fail-notes.log (S10a-S10d), s4_fail-notes.run1.log
- doctor: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s4-doctor-pw-gone.txt, s4-devices-pw-gone.json
- daemon log: s4-daemon.log, s4-daemon.run1.log
- control with the setting off (dictation also dead after a PipeWire restart, see QA81-4): s4b_fail_off-notes.log

## Traceability
- R-IDs: [R6]   scenario: S10   driver_rung: CLI over socket + pw-dump on private graph   viewport: n/a
