---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-76-re-run-a-failed-meeting-from-the-app.1 Implement Re-run a failed meeting from the app

## Description
TBD

## Acceptance
- [ ] TBD

## Done summary
Re-run on the meeting detail calls `meetings.recover` for a failed, partial, cancelled or stopped meeting that `meetings.status.recoverable` lists, and stays disabled with a tooltip reason otherwise (audio gone, recording, being transcribed, checking, or the reserved re-run on a completed meeting). The detail reads the meeting again once the recovery starts. The DettivoStyle ProgressBar clips its indeterminate sweep, which drew across the sidebar from the processing strip.

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: cdc76c1
- Tests: ctest --test-dir build/qt (86/86), scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, scripts/lint-copy.sh, cargo run -p xtask -- lint-file-length, scripts/check-docs.sh, scripts/test-check-docs.sh
- PRs: