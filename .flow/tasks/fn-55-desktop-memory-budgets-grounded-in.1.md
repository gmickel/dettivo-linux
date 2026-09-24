---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-55-desktop-memory-budgets-grounded-in.1 Implement measured desktop memory budgets

## Description
Implement the approved Thor memory contract of RSS ≤320 MiB and anonymous memory ≤128 MiB, preserve earlier failed evidence, and verify fresh release-profile native Wayland and Xvfb launches with the unchanged startup and theme timing limits.

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The approved Thor memory contract now enforces RSS at most 320 MiB (327680 KiB) and anonymous memory at most 128 MiB (131072 KiB), with MiB labels derived from the same constants and PSS uncapped. Fresh release validation passes five native Wayland and five Xvfb launches, plus routes and live theme timing; this recalibrates the allowance and claims no reduction in RAM use.

Requirement proof:
- R1/R3/R5: exact limits pass, one KiB above either fails, unavailable metrics remain failures, and measured values plus budget labels use correct KiB/MiB conversion. Both changed constants and stale labels were demonstrated red before their fixes; focused library tests pass.
- R2: all ten fresh samples pass, each after five idle seconds on connected Home with no capture, playback, download or analysis. Release binaries, hashes, actual display/scale/theme and process scope are retained. Native maxima are RSS 276868 KiB, anonymous 104084 KiB and first frame 241 ms; Xvfb maxima are RSS 229028 KiB, anonymous 82416 KiB and first frame 235 ms.
- R4/R6: the spec records Gordon's exact approval, active requirements/docs/ADR/gate references agree on 320/128, and the original proposal, old failed measurements and rejected experiments remain historical evidence. The first raised-limit series is retained as invalid metadata because its MiB labels were stale.
- R5: routes pass at 240 ms and theme application at 7 ms. The final exact mise exec just@1.58.0 -- just build test lint command passed, with 1000 Cargo tests and 78 CTest entries. Final report/documentation checks passed after evidence was staged. Optional packaging ShellCheck remains unavailable; no packaging source changed.

The full evidence is docs/reports/qa/2026-09-10-desktop-memory.md and docs/reports/qa/2026-09-12-approved-desktop-memory.json. Raw accepted commands and samples are under /tmp/fn55-raised-verified/. The final canonical log is /tmp/fn55-raised-canonical-verified.log. Earlier failed map/probe runs and the corrected zero-test selection remain recorded. Separate visual-baseline and installed-session approvals are outside this result.

GATE_SKIPPED:build-test-lint:green-receipt bf15c133 - baseline reused from prior post-gate pass
stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: f47114660f5132465c1d8e79a4834ef7240f2d11, bb9b20f524ebeb157bb9c4f84352bf853edb9339, 79f5b87f8f4051e2e273e02097d3f1ce08f89e0a, d19ba3b2cfd890ed89175a94ba3ab1bb2bd232c9
- Tests: GATE_SKIPPED:build-test-lint:green-receipt bf15c133 - baseline reused from prior post-gate pass, cargo test -p dettivo-qa scenarios::app_support::tests (baseline 3 library tests passed; /tmp/fn55-raised-baseline.log), INCONCLUSIVE: cargo test -p dettivo-qa --bin dettivo-qa scenarios::app_support::tests (running 0 tests; excluded from passing evidence; /tmp/fn55-raised-red.log), cargo test -p dettivo-qa --lib scenarios::app_support::tests (expected budget red, then 3 passed; /tmp/fn55-raised-red-lib.log and /tmp/fn55-raised-green.log), cargo test -p dettivo-qa --lib scenarios::app_support::tests (expected MiB-label red, then 3 passed; /tmp/fn55-raised-labels-red.log and /tmp/fn55-raised-labels-green.log), target/release/dettivo-qa evidence-map --write (469 R-IDs mapped; /tmp/fn55-raised-evidence-map.log), cargo test -p dettivo-qa --lib evidence_map::tests::the_checked_in_map_covers_every_rid_of_every_spec (1 passed; /tmp/fn55-raised-map-green.log), mise exec just@1.58.0 -- just build-release build-qt (exit 0; /tmp/fn55-raised-release-build.log), cargo build --release -p dettivo-qa (label correction release build exit 0; /tmp/fn55-raised-labels-release.log), mise exec just@1.58.0 -- just build test lint (map-reference failure retained at /tmp/fn55-raised-canonical.log), mise exec just@1.58.0 -- just build test lint (existing revision-probe failure retained at /tmp/fn55-raised-canonical-final.log), cargo test -p dettivo-qa --lib pack::binaries::tests::the_release_gate_refuses_a_debug_or_stale_binary_set (unchanged focused retry passed; /tmp/fn55-raised-binary-probe-retry.log), mise exec just@1.58.0 -- just build test lint (final exit 0, 1000 Cargo tests and 78 CTest entries; /tmp/fn55-raised-canonical-verified.log), Release app_memory.wayland and app_memory.xvfb (fresh 5 launches per mode, all pass; exact argv/env/exit codes at /tmp/fn55-raised-verified/commands.json), Release app_routes with DETTIVO_TIMING_GATE=1 (pass, first frame 240 ms; /tmp/fn55-raised-verified/routes), Release app_theme_live with DETTIVO_TIMING_GATE=1 (pass, apply 7 ms; /tmp/fn55-raised-verified/theme), python3 /tmp/fn55-approved-report.py (all raw metrics, budget labels, idle state and reference metadata assertions passed), scripts/check-docs.sh and scripts/test-check-docs.sh (final report checks passed; /tmp/fn55-raised-final-docs.log)
- PRs: