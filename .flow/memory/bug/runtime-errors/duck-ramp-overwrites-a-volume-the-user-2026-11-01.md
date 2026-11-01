---
title: "Duck ramp overwrites a volume the user sets during the ramp, then restores the o"
date: "2026-11-01"
track: bug
category: runtime-errors
module: crates/dettivod/src/media.rs ramp
tags: [qa, fn-85-delivery-that-leaves-the-desktop-as-it, media]
problem_type: runtime-error
symptoms: a user volume change landing between a ramp step's read and its set is overwritten; the take ends at the pre-take volume (4-5 of 16 offsets)
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/async-media-restore-outlived-the-2026-11-01]
---

## Problem
A user who changes the output volume while a `while_dictating = "duck"` ramp is running can lose that change: the ramp keeps going down to 20 %, and the restore then raises the sink back to the volume it had before the take. Found in live QA on a private PipeWire null sink.

## Steps to reproduce (cold)
1. Private profile (`scripts/qa/xvfb-session.sh`, isolated XDG dirs, own socket), `[audio] while_dictating = "duck"`, `DETTIVO_MOCK_MIC=jfk.wav`. A private null sink (`scripts/qa/audio-rig.sh up dettivo-qa-race85p1`) at 80 % stands in for the default sink: a `wpctl` shim first on the daemon's PATH maps `@DEFAULT_AUDIO_SINK@` to that sink's id and refuses any other mutation.
2. `dettivo dictation start`; wait until the daemon's first ramp step (`set-volume <id> 0.78`) appears in the shim log.
3. Wait 0 to 100 ms more, then set the sink to 66 % yourself (`wpctl set-volume <id> 0.66`).
4. `dettivo dictation cancel` (or stop) after about 0.8 s; read the sink volume once the take is idle.

Script: `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/tools/r5race.sh` (sweeps the offset 0 to 100 ms).

## Expected
R5: "`while_dictating = "duck"` lowers and restores the default sink smoothly, never overrides a volume the user set mid-take". Approach: "If the user changes the volume during the ramp, the ramp gives up." docs/dictation.md: "When you change the volume during the duck ramp, the ramp stops there, and a volume or a mute you set during the take stays after it."

## Actual
In 5 of 16 offsets (pass 1) and 4 of 16 (pass 2) the final volume is 0.80, not 0.66. The shim log shows the user's 0.66 landing between the ramp's `get-volume` (which still read the previous step) and its next `set-volume`, which overwrites it (`0.427 MARK user-0.66` then `0.443 set-volume 0.74`). The ramp then completes to 0.20, and the restore, which sees the ducked level, ramps back to 0.80. The other offsets give up correctly and keep 0.66. `crates/dettivod/src/media.rs` `ramp()` reads, compares, then sets, so a change between the read and the set is invisible to it.

The shim adds roughly 10 ms of bash per `wpctl` call, which widens the window; with the bare `wpctl` the window is smaller but still present in the read-then-set order.

## Evidence
- `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/runs/r5-race-p1/results.txt`, `wpctl.log`, `run.log`
- `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/runs/r5-race-p2/results.txt`, `wpctl.log`
- first seen: `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/runs/r5-media-p3/wpctl.log` and `volume-M4.log` (scenario M4)

## Traceability
- R-IDs: [R5]   scenario: S5-duck-user-change-during-ramp   driver_rung: CLI over the private socket + PipeWire null sink   viewport: n/a
