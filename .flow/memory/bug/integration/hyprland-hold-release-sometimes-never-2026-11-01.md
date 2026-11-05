---
title: Hyprland hold release sometimes never ends the take — drive hold step ran to the
date: "2026-11-01"
track: bug
category: integration
module: crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs
tags: [qa, fn-82-hold-to-talk-that-survives-lost, hotkeys]
problem_type: integration
symptoms: In 2/5 live drive runs the hold chord's release (and the double tap's tap release) had no effect; job_dict_1 duration_ms=11000 instead of ~2600; the drive's hold step still passed
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
In 2 of 5 live `dettivo-qa drive hotkeys_hyprland` runs, letting go of the hold chord did not end the take. Hyprland was running the generated Lua snippet, with real key presses from a uinput keyboard. The take recorded until the 11 s mock-microphone fixture ran out. In real use it would record until the next press or the 300 s limit. The drive's hold step still passed, because it waits up to 60 s for `idle` and cannot tell a take the release ended from one the fixture ended.

## Steps to reproduce (cold)
1. On the live Omarchy/Hyprland session, build the branch (e921988) and run `target/debug/dettivo-qa drive hotkeys_hyprland --evidence <dir>` several times.
2. In each run's `daemon.log`, read `dictation finished job=job_dict_1 ... duration_ms=`. The hold step keeps the chord down for 2.5 s.

## Expected
The hold step ends on release: "Hold to talk is a press binding that runs `dettivo --quiet dictation press` and a release binding that runs `dettivo --quiet dictation release`" (docs/hotkeys.md). A held take of about 2.5 s finishes at about 2.6 s.

## Actual
Runs 1 and 3: `job_dict_1 ... duration_ms=11000` (the fixture's end). In the same runs, the double tap's tap release also had no effect: no discard, the second press was logged as `hotkey press ignored: key repeat`, and the take ran to 11 s. Runs 2, 4 and 5: `duration_ms=2600..2680`, as expected.

A separate rig bound the same snippet shape on CTRL+ALT+SHIFT+F1 to a logging wrapper in front of the branch CLI and the isolated branch daemon, then held the chord 5 times through python-evdev. All 5 releases were spawned about 50 ms after key-up and answered `stopped` (s7-release-rig.log). The cause is therefore not established. The daemon does not log no-op edges at info, so the runs cannot show whether the release command never ran (Hyprland) or arrived and answered `none` (daemon).

## Evidence
- .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/qa-evidence/run-1793539833-3764215/hotkeys_hyprland.atspi/daemon.log (run 1)
- .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/qa-evidence/run-1793540078-3780158/hotkeys_hyprland.atspi/daemon.log (run 3)
- .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/s7-release-rig.log (the 5-of-5 control)

## Traceability
- R-IDs: [R3, R4]   scenario: S7   driver_rung: compositor + /dev/uinput   viewport: n/a (desktop)
