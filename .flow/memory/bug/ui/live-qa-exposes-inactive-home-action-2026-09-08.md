---
title: Live QA exposes inactive Home action and misleading model states
date: "2026-09-08"
track: bug
category: ui
module: qt
tags: [qa, fn-54]
problem_type: ui
symptoms: Live QA exposes inactive Home action and misleading model states
root_cause: (unspecified)
resolution_type: fix
last_updated: "2026-09-08"
---

# Live QA findings, initial build ba02db93

## QA-1. Home cannot start a meeting

P1. Home displays a disabled Start meeting button while the daemon is connected and idle. Meetings has a working start action. Reproduced in app_routes screenshots and the rendered Home on multiple palettes. The new AppHome::test_start_meeting_is_available_when_the_daemon_is_idle fails before the fix.

Evidence: run-1788851212-192755/app_routes.atspi/screenshot-home.png; visual/home/header.catppuccin-latte.1x.window.png; meeting-home-before.log.

## QA-2. Model variants incorrectly share warm state

P1. Models shows both tiny and tiny.en warm while only tiny.en is loaded. Observed in app_routes and the GUI pack's Models screenshot. A focused settings-model test reproduces the same state and fails before the fix. The loaded model path must identify only its own catalogue row.

Evidence: run-1788851212-192755/app_routes.atspi/screenshot-settings.models.png; gui/run-1788851247-197076/pack-gui/settings_roundtrip.models.atspi/screenshot-models.png; warm-before.log.

## QA-3. Hidden empty buttons enter the accessibility tree

P2. Both desktop drivers report unnamed buttons on first-run, Home, History and Meetings screens. Hidden optional actions remain exposed. The style test reproduces the visibility transition and fails before the fix.

Evidence: gui/run-1788851247-197076/pack-gui/gui-pack.json; hidden-before.log.

## QA-4. QA profile inherits the operator's host configuration override

P1 in the QA harness. The Agents page reads the real CODEX_HOME path despite an isolated HOME. Reproduced in settings_roundtrip.agents and app_routes; no Codex host write was performed. Host-specific configuration roots must be isolated along with HOME before any host integration actions run.

Evidence: gui/run-1788851247-197076/pack-gui/settings_roundtrip.agents.atspi/tree-agents.json; gui.log.

## QA-5. Settings happy-path fixture violates chunk validation

Harness mismatch. The generic integer sample 7 is not greater than the default overlap plus safety margin. The GUI correctly refuses it and config.get remains 300. Use a valid chunk length for the successful-write test and keep the invalid-value checks.

Evidence: gui.log; settings_roundtrip.meetings receipt.

## Coverage to execute

R1: first_run_fresh/provisioned/steps/resume, app_routes, all eight settings sections and invalid edits, history_seeded/states, meetings_seeded/live_gui/import_gui.
R2: osd_dictation, history_roundtrip, meeting_recovery/live, app_daemon, app_theme_live/theme_switch, isolated real-audio and insertion where supported.
R3: every visual-manifest cell, full windows inspected across fixture palettes, resized windows, keyboard navigation and the separate fn-38 evidence comparison.
R4: focused regression tests, affected scenarios repeated, final build/test/lint, QA receipt listing evidence and gaps.

All GUI actions use disposable seeded data. No artifact constitutes Gordon's fn-38 R6 approval.

## Update 2026-09-08

# Live desktop QA findings and remaining limits

The confirmed Home meeting, model-identity, state-refresh, accessibility, dialog-ownership, narrow-window, keyboard and QA-isolation defects from fn-54 are fixed. The committed report at `docs/reports/qa/2026-09-08-live-desktop.md` records their reproductions and regression evidence.

## QA-MEMORY. Idle app exceeds its footprint requirement

P1, confidence 100, pre-existing. A returning user opens Home with the seeded daemon, waits past five seconds and reads the app status. The expected idle RSS is below 60 MB (fn-17 R4 and docs/app.md). The app reports 236272 KB in `manual5/keyboard-route-final.json` and about 235556 KB in the earlier `media-lazy-routes` run. Both reproduce the requirement miss. The app remains usable.

Deferring media initialization reduced first frame from about 1556 ms to 255-256 ms and removed about 90 MB of RSS in these local runs with Release Qt and debug Rust binaries. Lazy page loading and a Vulkan renderer experiment did not establish sufficient further improvement and were reverted. Shared graphics-driver mappings contribute to RSS; the requirement itself remains unchanged. Further footprint work is open, with no release performance claim.

Evidence root `.flow/tmp/qa-live-20260908/`. See `native-trace/home.log`, `media-lazy-routes/`, `manual5/keyboard-route-final.json` and `manual5/home-final.png`.

## QA-VISUAL. Approved references no longer cover the rendered contract

P2, confidence 100, pre-existing and affected by intentional fn-54 text corrections. The five-palette, two-scale visual matrix reports missing approved references and repeated differences in Tokyo Night's pressed fill, first-run keycap layout and Agents REST text. The current pressed fill follows the active accent; the old reference used gold. The first-run keycap adds height relative to plain text. Agents names the implemented REST configuration instead of promising a future spec.

These differences remain unapproved. No baseline file or tolerance changed. Evidence includes the initial `visual/`, repeated `visual-final/` and final `visual-verified/` reports and their baseline/render/diff triples. Gordon's fn-38 R6 walkthrough remains required.

## Installed-session coverage

The GUI pack skips its two Omarchy-session steps in Xvfb. Read-only checks found no installed `dettivod.socket` and no Dettivo shell plugin in the user's current setup. Isolated native-driver interactions, Wayland renders and native OSD pacing ran; installation, socket activation and real shell-plugin loading did not. No personal configuration, model files or services changed.

## Update 2026-09-08

## Beauty iteration on PR #53

Tasks fn-54.2 through .6 fixed the evidenced Home/first-run alignment, initial transcript clipping, growing-tail/scroll interaction, analysis/footer overlap, long rail values, heavy content-panel chrome, compact model controls, keyboard focus/activation, minimum-width Meetings collisions and the translucent/narrow engine menu.

The menu finding was reproduced twice in an isolated live window. Its background alpha was 0.03, and a label-fit regression failed. The repair passes opacity, content-fit, window-bound and keyboard-activation tests. Fresh live selection persisted speech.meeting_model=tiny. See docs/reports/qa/2026-09-08-beauty-iteration.md and its unmodified before/after evidence images.

The broad GUI pack passed 84 routes; its two installed Omarchy checks remain skipped. The post-layout matrix has 393 passes, 107 failures and 90 first-approval entries among 590. Of the failures, 103 compare against earlier app-render approvals and four against original Black Gold artboard crops (Today and Models selections at both scales). These remain visible and require review; neither the artboards nor comparison thresholds were changed.

The existing memory finding remains open pending the approved fn-55 recalibration's implementation and measurement. Gordon's fn-38 R6 approval remains his checkpoint. No statement here closes either requirement or treats a Copilot quota refusal as a review.
