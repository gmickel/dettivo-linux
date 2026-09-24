# Live desktop QA after cleanup

## Goal & Context

Gordon requested a full live QA and visual pass after the cleanup merges, with confirmed defects fixed. Exercise the native app on isolated desktop profiles and inspect captured screens against the Studio references. Report exactly which behaviors and environments were verified.

## Acceptance Criteria

- **R1:** First run, Home, all settings sections, History and Meetings are driven against an isolated running daemon. Capture screenshots, accessibility trees and persisted responses for changes. Reproduce failures and fix confirmed product defects.
- **R2:** Dictation, cancellation, recovery, app reconnect and theme changes have live evidence. Record environmental limitations explicitly.
- **R3:** Inspect rendered surfaces across the five fixture palettes at 1x and 2x, plus full-window screenshots. Fix clipping, misleading state and accessibility defects without weakening visual comparison or replacing approved references to hide differences.
- **R4:** Re-run affected live scenarios and focused regression checks after fixes, then pass the repository build/test/lint gate. Publish a QA receipt with coverage and remaining limitations.

## Boundaries

- No installation into the user's live system, no changes to personal configuration or model files.
- Gordon's fn-38 R6 approval remains his checkpoint. An agent inspection cannot approve baselines under his name.
- No automatic start of the held cleanup backlog or GPU diarization feature. Findings needed for this requested QA may be fixed here.

## Quick commands

- `target/debug/dettivo-qa drive app_routes --driver atspi`
- `target/debug/dettivo-qa pack gui --continue`
- `target/debug/dettivo-qa visual`
- `make build test lint`
