---
satisfies: [R1, R2, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.1 Drive the desktop, fix reproduced defects, and verify the surfaces

## Description
Drive the app against isolated desktop profiles, reproduce and fix observed defects, and publish the live evidence and remaining release limitations.

## Acceptance
- [x] Exercise R1 routes and persisted write/error paths through the running app.
- [x] Capture R2 session, recovery, dictation and theme behavior, naming installed-session gaps.
- [x] Inspect R3 palette/scale renders and resized windows without replacing approved references.
- [x] Complete R4 focused checks and the repository gate, then publish the QA receipt and report.

## Done summary
Completed the live desktop QA pass and fixed reproduced Home actions, state summaries, exact model identity, accessibility, dialogs, narrow-window reachability, keyboard navigation and QA isolation/driving. Deferred media initialization reduced measured first-frame latency from about 1556 ms to 255-256 ms. ADR 0056 and the full report document the changes.

The 84-route GUI pack passed with no text findings and complete accessible naming for inspected controls. All 13 additional AT-SPI cases passed, as did the affected CUA reruns and final app-route/first-run checks. The full visual matrix recorded 478 passes, 22 differences and 90 missing approvals across 590 entries, with zero style findings.

The QA receipt remains NEEDS_WORK. Idle RSS is above the 60 MB requirement; Gordon's design walkthrough, baseline approvals and installed Omarchy/socket integration remain open. No personal configuration changes, installation, push, PR or release occurred. Held cleanup/GPU work remains unstarted.

Final verification is recorded in docs/reports/qa/2026-09-08-live-desktop.md and the fn-54 QA receipt. Raw evidence remains under .flow/tmp/qa-live-20260908/.
## Evidence
- Commits: 54672aba, c08eadc5, b88115f8, f0bca19e, 9c8d5e7f, 4351e0eb, 66869acd, 470525b4, 96f894ed
- Tests: make build test lint (serialized factory gate; /tmp/dtv-gate-qa-receipt.log), dettivo-qa pack gui --continue: 84 routes passed; 2 Omarchy steps skipped, 13 additional AT-SPI live scenarios passed; affected CUA rename/import reruns passed, Final app_routes and first_run_steps AT-SPI drives passed, dettivo-qa visual: 478 pass, 22 fail, 90 first approval; unchanged baselines and thresholds, Native OSD pacing: 2392 frames, zero dropped, p99 0.032 ms, Isolated real-audio check passed; correlation 0.99586, flowctl validate --spec fn-54-live-desktop-qa-after-cleanup --json, git diff --check
- PRs: