---
satisfies: [R1, R2, R3, R4]
---
# fn-42-diarization-on-the-gpu-the-cuda-build.1 Implement CUDA diarization drop-in with CPU fallback and measured QA

## Description
TBD

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# Optional CUDA diarization accepted for delivery

The optional CUDA drop-in, actual-provider reporting, prerequisite CPU fallback and configuration/package integration are delivered in the current combined fn-42/fn-56 branch. The default package remains CPU. No additional runtime or model changes were made for this delivery.

Gordon explicitly accepted delivery in the current state on 2026-09-10. The active acceptance section records that decision and retains the original criteria below it as historical evidence. The fourfold-throughput target remains failed. The source-built tuning retry reached 1.310x CPU throughput on the 300-second diagnostic; that is not a 4x pass or a universal speed claim. Existing benchmark reports preserve the original outcomes and source/model identities.

The delivery code passed the complete build/test/lint gate on 2026-09-10, including 74 Qt tests, strict scorer/clustering regressions, Clippy, packaging checks and documentation checks. Packaging lint reported ShellCheck unavailable. Earlier CUDA-feature, fallback, patch-drift and isolated packaging verification remain recorded in the benchmark reports.

This task is complete against the explicitly revised delivery criteria R1-R4. Main synchronization is pending the combined PR. No host installation, public release, new model experiment, or change to the held cleanup/visual-approval work is included.
## Evidence
- Commits: 88781d49d844e294272401a91a9355455088817a, 05e7f80a721910356d9773054fc0cb59738c8157, 11f798501f0491c91e698518cd7efa410277109d, bbb5e1702e6a270ecbf4b975b67546430168b3f4, ce6d3d93099ba808804d9804f0d9e5a26f05647f, a3077006fada494612b71743fbf9ee3a26cb18cd
- Tests: 2026-09-10: /home/gordon/.local/share/mise/installs/just/1.58.0/just build test lint exited 0 on the delivery code and updated documentation; all 74 Qt tests passed. Packaging lint reported ShellCheck unavailable., 2026-09-10: flowctl validate --spec fn-42 --json and --spec fn-56 --json passed; git diff --check and benchmark JSON parsing passed.
- PRs: