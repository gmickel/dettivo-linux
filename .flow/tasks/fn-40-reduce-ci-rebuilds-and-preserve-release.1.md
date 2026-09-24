---
satisfies: [R1, R2, R3, R4, R5]
---

# fn-40-reduce-ci-rebuilds-and-preserve-release.1 Optimize CI caches, scheduling and tested artifact reuse

## Description
# Reduce CI rebuilds and preserve release evidence

CI keeps the critical PR gate and headless smoke while reusing current compiled caches and tested release artifacts.

### Requirements
- R1: Cache identity includes installed toolchain and build configuration, with revision freshness and safe restore prefixes; assess Qt build caching.
- R2: Retain fast PR/main gate and smoke. Add stable fail-closed CI aggregate, sensible timeouts and concurrency. Keep extensive nightly regression and weekly fallback/drift evidence. Advisory uncalibrated benchmark cannot block release.
- R3: Release reuses verified build/package artifacts rather than rebuilding them through the rig. Preserve clean-install and exact-revision verification; publishing stays serialized and is never cancelled.
- R4: Update ADR and both repository instruction files to request Copilot on ready PR and explicit substantial rereview. Preserve existing review gate.
- R5: Run applicable workflow contracts and full just build test lint; record remote cold/warm gate plus nonpublishing rig evidence or exact blockers. Do not publish a release or replace physical acceptance.

### Quick commands
just build test lint
Workflow YAML/actionlint and focused policy contract validation.
## Acceptance
- [x] R1: Separate source downloads from per-job compiled Rust/Qt caches with installed-toolchain/configuration identity, revision freshness and compatible restore prefixes.
- [x] R2: Preserve PR/main gates, stable fail-closed aggregate, bounded jobs and concurrency, nightly regression and weekly fallback/drift coverage; benchmark remains advisory.
- [x] R3: Reuse one rig package artifact at release, verify checksums/revision and exact installed SHA, and serialize publishing without cancellation.
- [x] R4: Update ADR 0040, release documentation and both harness instruction files for ready/manual Copilot review while preserving the head-review gate.
- [x] R5: Record local full gate, workflow contracts and remote rehearsal evidence or exact blockers.

## Done summary
CI now restores compatible per-job Rust/Qt builds, saves revision-fresh caches, and exposes a stable fail-closed aggregate. The release workflow consumes the rig's sole checksum-bound package after exact-revision clean-install verification. Nightly regression and weekly fallback/drift coverage remain; uncalibrated benchmarking cannot block release. ADR 0040, release docs and both harness review-policy paragraphs reflect the change.

R1-R4 implemented. R5 local verification passed: 744 Rust tests, 59 Qt tests, the full lint/build gate, actionlint 1.7.12, and workflow contracts. The installed-revision regression first failed because an older same-version binary was accepted; all three cases now pass. The temporary workflow contract exercises all 16 aggregate result combinations, artifact round-trip plus revision/package/archive corruption, compatible cache prefixes, release dependencies, and timeouts.

baseline: green after tooling repair. The first compiler attempt failed with /tmp Disk quota exceeded (exit 101). TMPDIR under .flow/tmp/compiler resolved it; the complete baseline and verification each exited 0. Logs: .flow/tmp/baseline-resume.log, .flow/tmp/verify-resume.log, .flow/tmp/workflow-contracts.log and .flow/tmp/actionlint.log. The older shared /tmp/dettivo-ci-baseline.log was overwritten by another repository and was not used as evidence.

Remote cold/warm measurements and nonpublishing release-rig rehearsal are blocked: the live GitHub API reports ci, qa-weekly, release and rig disabled_manually. State receipt: .flow/tmp/remote-workflow-state.txt. No speedup is claimed, workflows were not enabled, and no release or physical acceptance was performed. The conductor owns any authorized enablement, push, PR and remote validation.

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: e22db78fcb4e4bee4dc6797d99a0832d11c9b1a2
- Tests: TMPDIR=$PWD/.flow/tmp/compiler mise exec just -- just build test lint: baseline and verify exit 0; 744 Rust tests, 59 Qt tests; initial /tmp quota failure repaired, scripts/packaging/test-installed-revision.sh: red before fix, green 3 cases after fix, /home/gordon/.local/state/ci-audit-20260905/tools/actionlint: exit 0, uv run --with pyyaml python .flow/tmp/check-ci-contracts.py: exit 0, scripts/packaging/lint.sh: exit 0; shellcheck unavailable on host, scripts/check-docs.sh: exit 0, git diff --check: exit 0, INCONCLUSIVE remote cold/warm CI and nonpublishing release rehearsal: all four workflows disabled_manually; no remote dispatch or release performed
- PRs: