---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8]
---
# fn-59-working-f9-shortcuts-after-onboarding.1 Implement reliable onboarding and polished Omarchy controls

## Description
TBD

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Implementation complete. The PR build is installed and clean shortcut setup passed; controlled native insertion and panel acceptance remain incomplete.

- Lua shortcut setup appends a missing include after existing configuration, preserves unrelated settings, checks reload/configuration errors and supports repeat setup. CLI and daemon use the same installer. Onboarding waits for setup, retains actionable failures and retries written-but-unsourced bindings.
- The plugin ships its singleton registration, keeps the waveform button visible and explicitly loads the packaged panel style without a session QML path override. Mode segments fill their track equally; action buttons use the shared flat theme.
- Updated hotkey/plugin docs, ADRs and unreleased changelog. Already loaded plugin code may remain cached after rescan; applying an upgrade can require a later shell restart. The implementation stage did not restart or install desktop components.

Verification completed in separate stages, not one successful all-in-one command. The build stage of `just build test lint` passed (`/tmp/fn59-isolated-full-gate-final.log`). Full workspace Rust tests, all 79 CTest cases and the scoring tests passed in `just test lint` (`/tmp/fn59-isolated-test-lint.log`); lint then caught the added panel test exceeding 300 lines. The unchanged regression was moved to `tst_bar_controls.qml` and passed separately (3 checks, `/tmp/fn59-bar-controls-split.log`). A subsequent `just lint` passed with exit 0 (`/tmp/fn59-isolated-lint.log`). The isolated command wrapper is `/tmp/fn59-isolated-gate.sh`.

The final test environment used private runtime/config/data/cache directories and D-Bus, unreachable Pulse/PipeWire endpoints, and no desktop display/compositor sockets. Only public tiny.en and JFK fixtures were copied into private data. Live audio-service branches were unavailable; these results do not satisfy native acceptance. Focused red-to-green checks covered the missing include, visible widget, absent QML style path, panel geometry and onboarding failure/retry.

An earlier verification attempt was stopped (exit 143) after an insufficiently isolated audio test temporarily changed the live default source. Its restoration completed; the conductor checked the current default read-only. Subsequent setup failures were missing private tool configuration/fixture companions, not code passes. The only code-caused gate failure was the test-file length, corrected by splitting it.

Native QA resumed after Gordon authorized reinstall and retest. The installed 0.1.0-1.57 package matches commit 36812db64cfa, and package integrity reports zero altered files. Clean Keys Continue activated the missing include and both F9 bindings; the fresh plugin loaded without session QML-path overrides. The original input setting and widget placement were restored after interrupted takes. Controlled insertion, panel interaction and restart persistence still need an idle desktop interval. Keep the PR open.

baseline: none (the spec defines no Quick commands).
stage: implement - ran
stage: plan-sync - skipped(config: disabled)
stage: QA - ran (partial; remaining native checks await a stable desktop)
stage: impl-review - skipped(config: REVIEW_MODE=none)

Blocked:
The PR build is installed, package integrity and clean shortcut setup passed, and the fresh plugin loads without the local workarounds. Controlled F9 insertion, panel interaction and restart persistence remain incomplete because focus/workspace changed during takes. Waiting for the requested idle desktop interval. Recording is cancelled; test audio/input helpers are cleaned up and normal input/widget placement restored. Keep PR #57 open.

Blocked:
PR #57 merged and the combined #57/#58 build is installed as dettivo-bin 0.1.0-1.58 at Gordon's request. Package integrity, live app/daemon identity, native settings rendering, preserved configuration and both F9 bindings were verified. Controlled native F9 insertion, panel interaction and restart-persistence acceptance remain pending. The earlier request to keep the PR open is superseded by the explicit merge instruction; no native acceptance pass is claimed.
## Evidence
- Commits:
- Tests:
- PRs:
