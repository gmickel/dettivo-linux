---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-56-reliable-automatic-meeting-diarization.1 Calibrate and implement reliable automatic diarization with held-out accuracy gates

## Description
TBD

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec directly.

## Done summary
# Current native calibration accepted for delivery

The current native automatic-clustering calibration, English ERes2Net catalogue identity and configuration integration are delivered with the fn-42 optional CUDA foundation in one branch. The code retains explicit speaker-count support, CPU availability, prerequisite GPU fallback, cancellation and personal-data boundaries. This delivery adds no new inference algorithm or model.

Gordon explicitly requested delivery of fn-42 and fn-56 in their current state on 2026-09-10. The active criteria R1-R5 now record this accepted scope; the original criteria remain unchanged in the historical section. Both held-out rounds still failed. The second round scored 17.72% pooled CPU DER with two exact counts out of seven, and its complete CUDA comparison remains unavailable after the recorded infrastructure interruption. The paired fourteen-recording native development comparison remains evidence of that narrower CPU/CUDA validation only.

The strict reference scorer, pinned input/configuration identities, upstream reproduction and failed alternative-model experiments remain documented. No alternative was promoted into the product. ADRs 0057/0058 and the accuracy report distinguish accepted delivery from a high-accuracy pass. Restricted external experiment results remain outside the repository and PR.

The delivery code passed the full build/test/lint gate on 2026-09-10, including all 74 Qt tests, strict scorer/clustering regressions, Clippy, packaging and docs checks. Packaging lint reported ShellCheck unavailable. Flow validation, diff whitespace checks and benchmark JSON parsing passed.

This completes the accepted delivery scope, not a production-readiness certification. Main synchronization is pending the combined PR. The held cleanup specs, memory-budget work, visual approvals, live installation and public release remain outside this change.
## Evidence
- Commits: 88781d49d844e294272401a91a9355455088817a, df892dd1b4905bac3d24198d1a5c233ed4576230, ab158908aaf221cdc86c9141e3700cd29e9d78a8, 1cdf88a7810a748d4557b5f831b0db1878d994a2, c01d397ab4a36e8debfad1a04fb9331489e4b649, ec87e84a961d50fa895809d627c8256925ec3e85, 832a32e81a973a8ae687d631eb69447b4fdb7c2e, 517d420ed5741d97f761776dc06ce5c871fd4f80, 9971ff1913fd15ab6b6c3cedb20006268f101aa1, 23cfd5812b1c146ad2d45cda6cdc95e38ee8670c, a02aac3069d61a43af8cec583ca84a01de1b4d8f, 55c272dfa44731d42988749289dad4d4d123bdbd, c2a49b322184d1d394706dbea636057b49afed25, 05e7f80a721910356d9773054fc0cb59738c8157, 11f798501f0491c91e698518cd7efa410277109d, bbb5e1702e6a270ecbf4b975b67546430168b3f4, ce6d3d93099ba808804d9804f0d9e5a26f05647f, a3077006fada494612b71743fbf9ee3a26cb18cd
- Tests: 2026-09-10: /home/gordon/.local/share/mise/installs/just/1.58.0/just build test lint exited 0 on the delivery code and updated documentation; all 74 Qt tests passed. Packaging lint reported ShellCheck unavailable., 2026-09-10: flowctl validate --spec fn-42 --json and --spec fn-56 --json passed; git diff --check and benchmark JSON parsing passed.
- PRs: