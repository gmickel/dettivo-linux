---
title: "Maintainer can't prove the double-tap lock on Hyprland — hotkeys_hyprland drive "
date: "2026-11-01"
track: bug
category: integration
module: crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs
tags: [qa, fn-82-hold-to-talk-that-survives-lost, hotkeys]
problem_type: integration
symptoms: "dettivo-qa drive hotkeys_hyprland fails 5/5 at the double tap: second press starts a plain held take (job_dict_5 duration_ms=740) instead of locking; the lost-release step never runs"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/integration/at-spi-history-input-loses-a-leading-2026-09-10, bug/integration/sway-repeats-a-held-bindsym-which-the-2026-11-01]
---

## Problem
A maintainer running the hold-to-talk proof on Omarchy (`dettivo-qa drive hotkeys_hyprland`) gets a failure every time: the drive's double tap never locks, so it never reaches its lost-release step. The daemon behaves as specified; the drive's own key timing falls outside the 300 ms double-tap window.

## Steps to reproduce (cold)
1. On the live Omarchy/Hyprland session, in the worktree at e921988: `cargo build -p dettivod -p dettivo-cli -p dettivo-qa -p dettivo-engine-whisper`.
2. `target/debug/dettivo-qa drive hotkeys_hyprland --evidence .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/qa-evidence`
3. Wait for it to finish (about 26 to 34 s).
4. Read the result line and the scenario's `daemon.log`.

## Expected
R4: "Both have unit tests on the state machine and a QA drive with a synthetic missing release." R5: "Tap, then press again within 300 ms, locks recording." The drive is documented to see `recording, cancelled, idle, recording, recording` for the double tap, then the lost-release step.

## Actual
5 of 5 runs fail at the double tap. In runs 2, 4 and 5: `waited for 16 states ending in "recording"`. The tap is dropped, but the second press starts an ordinary held take, and the release finishes it (`job_dict_5 ... duration_ms=740`) instead of locking it. The drive's `Keyboard` sleeps 40 ms after every key edge, so the gap from the F-key coming up to going down again is 3 modifier releases + 100 ms + 3 modifier presses, about 380 ms. The daemon logs show the second press reaching the daemon 360 to 395 ms after the tap's discard.

The same gap sent through the CLI verbs reproduces it. With a 0.15 s gap the release answers `locked`. With 0.38 s it answers `stopped` (s7-double-tap-gap-cli.log). The daemon is correct; the drive's comment ("The edges stay 80 to 100 ms apart") does not count the per-modifier sleeps. The lost-release step after it never runs on a live desktop. The task's done summary lists the live drive as a follow-up, so it had not been run.

## Evidence
- console: .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/s7-hotkeys-hyprland-drive-run{1..5}.log
- daemon logs: .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/qa-evidence/run-*/hotkeys_hyprland.atspi/daemon.log, events.jsonl, receipt.json
- timing proof: .flow/tmp/qa-fn-82-hold-to-talk-that-survives-lost/s7-double-tap-gap-cli.log
- code: crates/dettivo-qa/src/scenarios/hotkeys_hyprland.rs (double tap block), crates/dettivo-qa/src/keys.rs (40 ms per edge)

## Traceability
- R-IDs: [R4, R5]   scenario: S7   driver_rung: compositor + /dev/uinput (no a11y driver)   viewport: n/a (desktop)
