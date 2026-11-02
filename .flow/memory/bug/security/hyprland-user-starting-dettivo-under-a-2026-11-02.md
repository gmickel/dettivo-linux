---
title: Hyprland user starting Dettivo under a lock gets the microphone captured briefly
date: "2026-11-02"
track: bug
category: security
module: crates/dettivod/src/ready_screen.rs
tags: [qa, fn-81-catch-the-first-and-last-words-of-every, ready_screen]
problem_type: security
symptoms: Daemon started while the compositor reports locked opens the ring for ~250-300 ms (2 of 4 runs) before closing it
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/security/omarchy-user-with-keep-microphone-ready-2026-11-02]
---

## Problem
A Hyprland user with `[audio] keep_microphone_ready = true` whose daemon starts while the screen is already locked (a crash restart by systemd, a socket activation under the lock) gets the microphone captured for about a quarter second while the session is locked. The capture closes on its own at the next reconcile tick; ring audio is not written anywhere.

## Steps to reproduce (cold)
1. Private PipeWire with a virtual source `dq81-mic` (no real device), a stand-in logind on a private system bus (LockedHint=false), and a stand-in compositor on `/tmp/dq81r/wl-0` that offers only `hyprland_lock_notifier_v1` and reports `locked` (it sends `locked` at once to a new lock notification, as Hyprland does for a session that is locked already).
2. Follow the graph with `pw-dump -m` (pwmon.py).
3. Start an isolated `dettivod` with `keep_microphone_ready = true` and `WAYLAND_DISPLAY=/tmp/dq81r/wl-0`.
4. Read the daemon log and the graph during the first second.

## Expected
R5: "Ring audio never touches disk or any client ... and the ring closes on disable, lock and pause." docs/dictation.md: "The daemon closes the capture ... while the screen is locked". A daemon that starts under a lock should never open the ring until the compositor reports unlocked.

## Actual
The daemon logs `session lock changed source=Compositor locked=Some(false)`, then `locked=Some(true)` 50 us later, then `microphone ready: open`, and closes it about 300 ms later (`closed why="no longer wanted"`). On the private graph a dettivod capture stream linked to `dq81-mic` exists from 1793612170.741 to 1793612170.985 (about 245 ms) while the stand-in reports locked. Reproduced in 2 of 4 runs (log in runs 2 and 4, graph in run 4); runs 1 and 3 won the race and stayed closed. `audio.devices` then reads `closed: the session is locked` correctly.

Likely cause (read after the live capture): `crates/dettivod/src/ready_screen.rs:70-72` reports `false` ("Unlocked until the compositor says otherwise") as soon as it binds the notifier, before the compositor's initial `locked` can arrive, and the ready loop acts on that value.

## Evidence
- daemon log: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r2/s9b-daemon-4.log (lines 1-5)
- graph: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r2/s9b-pw-graph-4.jsonl
- notes: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r2/s9b_lock_edges-4-notes.log
- script: scratchpad h2/s9b_lock_edges.sh

## Traceability
- R-IDs: [R5]   scenario: S9   driver_rung: CLI + pw-dump on a private graph   viewport: n/a
