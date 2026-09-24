# Reduce CI rebuilds and preserve release evidence

## Goal and context

CI keeps the critical PR gate and headless smoke while reusing current compiled caches and tested release artifacts.

## Acceptance criteria
- **R1**: Cache identity includes installed toolchain and build configuration, with revision freshness and safe restore prefixes; assess Qt build caching.
- **R2**: Retain fast PR/main gate and smoke. Add stable fail-closed CI aggregate, sensible timeouts and concurrency. Keep extensive nightly regression and weekly fallback/drift evidence. Advisory uncalibrated benchmark cannot block release.
- **R3**: Release reuses verified build/package artifacts rather than rebuilding them through the rig. Preserve clean-install and exact-revision verification; publishing stays serialized and is never cancelled.
- **R4**: Update ADR and both repository instruction files to request Copilot on ready PR and explicit substantial rereview. Preserve existing review gate.
- **R5**: Run applicable workflow contracts and full just build test lint; record remote cold/warm gate plus nonpublishing rig evidence or exact blockers. Do not publish a release or replace physical acceptance.

## Quick commands
just build test lint
Workflow YAML/actionlint and focused policy contract validation.
