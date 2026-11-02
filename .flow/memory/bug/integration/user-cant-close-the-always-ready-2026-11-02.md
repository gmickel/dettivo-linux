---
title: "User can't close the always-ready microphone by pausing Dettivo — no pause verb "
date: "2026-11-02"
track: bug
category: integration
module: crates/dettivod/src/ready.rs
tags: [qa, fn-81-catch-the-first-and-last-words-of-every, dictation]
problem_type: integration
symptoms: "R5 asks the ring to close on pause; Dettivo has no pause action, only the setting"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/integration/at-spi-history-input-loses-a-leading-2026-09-10, bug/integration/cli-user-cant-set-a-vocabulary-table-as-2026-11-01, bug/integration/hyprland-hold-release-sometimes-never-2026-11-01, bug/integration/maintainer-cant-prove-the-double-tap-2026-11-01]
---

## Problem
fn-81 R5 requires the always-ready ring to close when the user pauses Dettivo. Dettivo has no pause action, so there is nothing a user can do short of turning `keep_microphone_ready` off (a config write) to close the ring for a while.

## Steps to reproduce (cold)
1. Isolated profile daemon with `[audio] keep_microphone_ready = true`.
2. Look for a pause: `dettivo --help`, `dettivo dictation --help`, the contract methods, Settings. None exists.
3. `crates/dettivod/src/ready.rs::closed_because` closes only for the setting, a lock and a meeting.

## Expected
R5: "the ring closes on disable, lock and pause." Approach: "The ring closes when dictation is disabled, the session locks, or the user pauses Dettivo."

## Actual
No pause surface exists. ADR 0092 records "pause has nothing to attach to". The setting itself works as the off switch (verified live: off closes the capture within 0.6 s).

## Evidence
- .flow/tmp/qa-fn-81-catch-the-first-and-last-words-of-every/s1_pipewire-notes.log (S7e setting off -> state off, capture gone)
- docs/adr/0092 Situation and Consequences; crates/dettivod/src/ready.rs closed_because

## Traceability
- R-IDs: [R5]   scenario: S7-pause   driver_rung: CLI   viewport: n/a
