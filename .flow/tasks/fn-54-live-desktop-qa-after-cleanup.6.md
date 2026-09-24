---
satisfies: [R1, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.6 Make engine menus opaque and readable

## Description
Live minimum-window keyboard QA reproduced an engine menu painted transparently over page text with indistinguishably truncated choices. Fix shared menu background opacity and fit content width within the window; preserve keyboard selection and scroll behavior. Original baselines remain unchanged.

## Acceptance
The menu hides underlying page text, fits full representative engine labels when the window permits, stays inside the window at minimum size, and remains keyboard selectable. Focused regression tests fail before and pass after; capture live before/after evidence and rerun affected Qt/style/live checks.

## Done summary
The live engine menu now covers the page beneath it and shows complete representative model names. Its width follows item content within the window, and its height is bounded so long menus scroll. Keyboard activation still selects once.

The isolated live walk reproduced translucent text-over-text and truncated names twice. Focused tests failed before the fix with background alpha 0.03 instead of 1 and insufficient label width. All three menu regressions now pass, including oversized content bounds. A fresh 969x410 live screenshot shows the repaired menu; keyboard selection was confirmed through the daemon's persisted speech.meeting_model value. Evidence is in .flow/tmp/beauty-20260908/menu-{red,bounds,lint,tests}.log and .flow/tmp/qa-live-20260908/{beauty-final,beauty-menu}/.

Qt build, all 74 CTest checks, QML formatting/lint, token lint, accessibility-name lint and git diff --check passed. No original artboard, approved baseline, threshold or active memory budget changed. The conductor owns the final repository gate and QA receipt.

stage: impl-review - skipped(config: repository fast-build mode)
stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 5b5e953077d01c3f552fd57485f100bf34be6c16
- Tests: make build-qt, MenuLayout regressions: 2 failures before; 3 tests passed after, plus setup/cleanup, ctest --test-dir build/qt --output-on-failure: 74/74 passed, scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, git diff --check, Fresh live engine menu at 969x410; keyboard selection persisted speech.meeting_model=tiny
- PRs: https://github.com/gmickel/dettivo-linux/pull/53