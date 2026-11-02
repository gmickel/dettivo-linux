---
title: Omarchy user with keep_microphone_ready on can't get the microphone closed by lo
date: "2026-11-02"
track: bug
category: security
module: crates/dettivod/src/ready_session.rs
tags: [qa, fn-81-catch-the-first-and-last-words-of-every, dictation]
problem_type: security
symptoms: "Ring closes only on logind LockedHint/Active; Omarchy's Quickshell lock sets neither, so the microphone stays captured while locked"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
An Omarchy user with `[audio] keep_microphone_ready = true` who locks the screen leaves the always-ready microphone open: the capture keeps running into the in-memory ring and the desktop's microphone indicator stays lit for as long as the screen is locked. fn-81 R5 requires the ring to close when the session locks.

## Steps to reproduce (cold)
1. Isolated profile daemon (fn-81 QA rig: private Xvfb :181, private PipeWire with only virtual sources, private system bus carrying a stand-in logind), `[audio] keep_microphone_ready = true`.
2. `dettivo call audio.devices '{}'` -> `microphone_ready.state = "open"`, one Dettivo capture on the virtual microphone (pw-dump).
3. Set logind `Session.LockedHint = true` -> within 150 ms the ring closes (`closed`, "the session is locked") and the capture leaves the graph. Set it back -> it reopens. `Active = false` closes it too.
4. On the live Omarchy desktop (read-only inspection, the session was not locked): the locker is Quickshell's `WlSessionLock` (ext-session-lock-v1) started by `omarchy-system-lock` -> `omarchy-shell lock lock`. Nothing in `/usr/share/omarchy/shell` or `/usr/bin/omarchy-*` sets logind's `LockedHint` or calls `loginctl lock-session`, and the quickshell binary carries no `LockedHint` string. docs/qa.md:148 records the same: "`loginctl LockedHint` alone does not reflect Quickshell's session lock".
5. `crates/dettivod/src/ready_session.rs` follows only `LockedHint` and `Active`, so on Omarchy the ring never sees the lock.

## Expected
R5: "Ring audio never touches disk or any client. ... the ring closes on disable, lock and pause." Approach: "The ring closes when dictation is disabled, the session locks, or the user pauses Dettivo." Omarchy/Hyprland is the product's primary desktop.

## Actual
The ring closes only on logind's LockedHint/Active. Omarchy's lock changes neither, so with the setting on the microphone stays captured (indicator lit, ring filling) while the screen is locked. ADR 0092 records this as a known consequence rather than meeting R5.

## Evidence
- live lock path: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s1_pipewire-notes.log (MARK S7a-S7d), s1-pw-graph.jsonl, s1-daemon.log ("session lock changed blocked=true" -> "microphone ready: closed")
- Omarchy lock detectability: .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/r5-omarchy-lock-detectability.log
- code: crates/dettivod/src/ready_session.rs (LockedHint, Active only); docs/adr/0092 Consequences
- Omarchy exposes its own lock state (`omarchy-shell lock status` -> `.secure`; ext-session-lock), not queried live by this run.

## Traceability
- R-IDs: [R5]   scenario: S7-omarchy-lock   driver_rung: CLI over socket + pw-dump on private graph   viewport: n/a
