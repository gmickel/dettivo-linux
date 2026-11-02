---
title: Dictation user can't record after a PipeWire restart — daemon keeps a dead conne
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-audio (AudioService)
tags: [qa, fn-81-catch-the-first-and-last-words-of-every, audio]
problem_type: runtime-error
symptoms: After a PipeWire restart every dictation is cancelled as silence until dettivod restarts; also with keep_microphone_ready off
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/doctor-says-microphone-ready-open-after-2026-11-02]
---

## Problem
After a PipeWire restart (`systemctl --user restart pipewire`, an update, an audio fix) the daemon never captures audio again until it is restarted itself. Every dictation records nothing and ends `cancelled` with "silence: level"; the user only sees dictations that do nothing. This happens with `keep_microphone_ready` off as well, so it predates fn-81.

## Steps to reproduce (cold)
1. Isolated profile daemon on a private PipeWire graph, default config (setting off). A dictation works: `release` -> `stopped`.
2. Stop the private pipewire + wireplumber; `press`/`release` -> `CONFLICT sessionCancelled` (silent take).
3. Start PipeWire again, wait 6 s, feed the virtual microphone; `press`, 1.5 s, `release` -> `CONFLICT sessionCancelled`, zero Dettivo captures in pw-dump.
Reproduced in s4b (setting off) and twice in s4 (setting on).

## Expected
Dictation keeps working once PipeWire is back (R6's spirit: device changes recover without user action); at minimum the failure names PipeWire, not silence.

## Actual
The daemon keeps its dead PipeWire connection; `audio.devices` still says `pipewire: true`; takes are silent.

## Evidence
- .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s4b_fail_off-notes.log, s4b-daemon.log
- .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s4_fail-notes.log, s4-devices-pw-gone.json

## Traceability
- R-IDs: [R6] (adjacent; pre-existing)   scenario: S11   driver_rung: CLI + pw-dump   viewport: n/a
