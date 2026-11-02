---
title: Hyprland user with another display's instance in the environment sees it reporte
date: "2026-11-02"
track: bug
category: ui
module: crates/dettivo-insert/src/instance.rs
tags: [qa, fn-91-dettivo-survives-a-compositor-restart, insert]
problem_type: ui
symptoms: doctor says the environment's instance 'is stale (the compositor restarted)' and the switch log says from="none" when the environment names another display's live instance
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/agent-guide-exit-code-table-omits-exit-2026-11-01, bug/ui/cli-user-reading-config-keys-cant-learn-2026-11-01, bug/ui/cli-users-comma-less-last-vocabulary-2026-11-01, bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/english-first-run-user-sees-the-models-2026-11-01, bug/ui/keyboard-user-tabbing-to-the-english-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-keeps-seeing-a-vocabulary-2026-11-01, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
A Hyprland user whose daemon environment names a live Hyprland instance of another Wayland display (for example a nested Hyprland) now gets the right behaviour (this display's instance is used, the other is never queried), but the diagnostics describe it as a compositor restart: `dettivo doctor` says the environment's instance "is stale (the compositor restarted)", and the journal's switch line says `from="none"` instead of naming the environment's signature.

## Steps to reproduce (cold)
1. Private XDG_RUNTIME_DIR with a stand-in Wayland compositor on `wl-0`, a stand-in Hyprland instance `other` whose hyprland.lock names `wl-9`, and `live` whose lock names `wl-0` (QA scratch harness, scenario `s_r2_stale`, third world).
2. Start the real `dettivod` (head 6ff03ff) with `WAYLAND_DISPLAY=wl-0`, `HYPRLAND_INSTANCE_SIGNATURE=other`, `[insert] backend = "virtual_keyboard"`.
3. `dettivo --json insert --text x`, then `dettivo doctor`, then read the daemon log.

## Expected
ADR 0099: "The log says `hyprland: switched to the live instance` once per switch, with both signatures." The doctor row should not attribute the mismatch to a restart.

## Actual
Journal: `INFO hyprland: switched to the live instance from="none" to=live`. Doctor: `compositor Hyprland instance live; the environment's other is stale (the compositor restarted)`. Insertion itself is correct (typed `x`, other=0 queries, live=2). Reproduced in two separate runs.

## Evidence
- console: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/recheck-run2/S2-r2-env-other-live.log (line with "doctor compositor row")
- daemon log: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/recheck-run1/S2-r2-env-other-live.dettivod.log and recheck-run2/ (same)

## Traceability
- R-IDs: [R2]   scenario: S2e   driver_rung: CLI + stand-in compositor/IPC on private sockets   viewport: n/a
