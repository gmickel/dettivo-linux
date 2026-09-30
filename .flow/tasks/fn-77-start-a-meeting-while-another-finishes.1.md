---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8]
---
# fn-77-start-a-meeting-while-another-finishes.1 Implement Start a meeting while another finishes, and go back to the GPU once it has room

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Start meeting now starts the next meeting while the last one transcribes, analyses or assigns speakers; it reopens the live view only while a meeting is recording or stopping (`MeetingLiveModel::capturing`, used by NewMeetingRail and the Home start). The finishing meeting's progress, completion and failure leave the new live meeting intact.

An engine on the CPU after a GPU crash or OutOfDeviceMemory error tries the GPU again at the start of its next request once a hold passes: 2 minutes, doubling on each repeat failure up to 30, reset when a request runs on the GPU, cleared on unload or a changed load. Logic in `crates/dettivo-speech/src/supervisor_recovery.rs`; the first hold is `Settings::gpu_retry_hold` so tests use milliseconds.

Redaction and the crash warning keep libstdc++'s terminate line (plain C++ type) and its what() line when the message starts with `vk::` or `ggml`.

ADR 0080 amended, ADR index row updated, CHANGELOG entry under Unreleased. R7 (live desktop GPU-filling check) left for the conductor.

baseline: none recorded pre-edit (focused suites run after edits)
stage: plan-sync - skipped(config: planSync.enabled=false)
stage: impl-review - skipped(config: fast-build mode, review backend none)
## Evidence
- Commits: fdaef46937f4e84fbeb568c2784dff7fda5e7bd1
- Tests: cargo test -p dettivo-speech -p dettivo-engine-proto, cargo clippy --workspace --all-targets -- -D warnings, cargo fmt --all --check, cargo run -q -p xtask -- lint-file-length, scripts/check-docs.sh, scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, ctest --test-dir build/qt (86/86)
- PRs: