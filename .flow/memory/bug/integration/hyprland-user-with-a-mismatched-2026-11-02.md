---
title: Hyprland user with a mismatched environment gets another display's compositor pr
date: "2026-11-02"
track: bug
category: integration
module: crates/dettivo-insert/src/instance.rs
tags: [qa, fn-91-dettivo-survives-a-compositor-restart, insertion]
problem_type: integration
symptoms: HYPRLAND_INSTANCE_SIGNATURE naming a live instance of another Wayland display is queried for the focused window; the daemon's own display's instance is never tried
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/integration/at-spi-history-input-loses-a-leading-2026-09-10, bug/integration/cli-user-cant-set-a-vocabulary-table-as-2026-11-01, bug/integration/hyprland-hold-release-sometimes-never-2026-11-01, bug/integration/maintainer-cant-prove-the-double-tap-2026-11-01, bug/integration/user-cant-close-the-always-ready-2026-11-02]
---

## Problem
A Hyprland user whose daemon environment names a live Hyprland instance of a different Wayland display (for example a nested or second Hyprland of the same user) gets that instance's focused window probed, and the daemon types into its own display on the strength of the other compositor's answer. The fn-91 boundary says "Never connect to or act on a compositor other than the user's current session's (match the Wayland display)"; the discovery added in fn-91 checks the display only for instances it finds itself, not for the instance the environment names.

## Steps to reproduce (cold)
1. Private XDG_RUNTIME_DIR with a stand-in Wayland compositor on `wl-0`, a stand-in Hyprland instance `live` whose hyprland.lock names `wl-0`, and a stand-in instance `other` whose hyprland.lock names `wl-9` (QA scratch harness, scenario `s_r2_stale`, third world).
2. Start the real `dettivod` with `WAYLAND_DISPLAY=wl-0` and `HYPRLAND_INSTANCE_SIGNATURE=other`, `[insert] backend = "virtual_keyboard"`.
3. `dettivo --json insert --text x`, then `dettivo --json insert target`.

## Expected
Spec boundary: "Never connect to or act on a compositor other than the user's current session's (match the Wayland display; never guess across users or seats)." ADR 0099 decision: "The environment's signature is a hint. The probe connects to the instance in use, which at first is the environment's." The observed behaviour matches the ADR, so this is a design gap that predates fn-91, not a regression.

## Actual
`other` (display wl-9) answered 2 focus queries, `live` (wl-0) answered 0; `insert target` reports `compositor: {signature: "other", environment_stale: false}`, and the text was typed into wl-0. Reproduced in two separate runs.

## Evidence
- console: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/S2-r2-env-other-live.log
- daemon log: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/S2-r2-env-other-live.dettivod.log
- results: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/results-all.log (S2e-info)

## Traceability
- R-IDs: [R2] (edge outside its wording)   scenario: S2e   driver_rung: CLI + stand-in compositor/IPC on private sockets   viewport: n/a
