---
satisfies: [R1, R2]
---
# fn-57-keep-benchmark-indexing-scoped-to-suite.1 Filter benchmark index inputs without hiding malformed suite reports

## Description
Repair bench/render.rs report discovery and add focused regressions for unrelated accuracy JSON and malformed suite JSON. Add the matching evidence routes. Preserve all measurements and runtime behavior.

## Acceptance
R1 and R2 pass focused regressions; the next benchmark filing and final build/test/lint gate pass.

## Done summary
Benchmark filing now ignores unrelated accuracy JSON while rejecting malformed files named as suite reports. The focused regression first reproduced the missing generated_unix failure, then passed with the filter. Both complete CPU/GPU reports and their meeting blocks are committed; the CPU filing exercised the repaired README reader against the real directory.

Verification passed with 1,013 Rust tests, 79 Qt tests and the repository lint/documentation gates through just build test lint. Both meeting packs passed all 14 required steps. GPU workload attribution remains explicitly unproven, and the CPU tier ran on this desktop rather than a CPU-only VM. The final release receipt remains a separate release-stage gate.
## Evidence
- Commits: 19613bb5be85decc2d005b26921cb5cf938eb60b, 1ac14896
- Tests: cargo test -p dettivo-qa the_readme_ignores_accuracy_reports_but_rejects_malformed_suite_reports, just bench-cpu target/release, just qa-pack-meetings target/release, just qa-pack-meetings-cpu target/release, just build test lint
- PRs: