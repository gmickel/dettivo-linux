---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20]
---
# fn-50-cleanup-qa-isolation-and-honest.1 Cleanup: QA isolation and honest assertions, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-50 cleanup receipt

| R-ID | Outcome | Evidence |
|---|---|---|
| R1 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; strict replay stateful skips are checked against named daemon integration tests. |
| R2 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; `mcp check` launches a served child and performs initialize, tools/list and get_status framing checks. |
| R3 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; acceptance coverage runs the language pipeline through Raw and later stages. |
| R4 | fixed | `1a0a5210362f5a53b26bbd303c2f2a21a9e2bc3f`; audio-rig cleanup unloads the exact owned module and rolls back when its monitor is absent. |
| R5 | fixed | `9b86e10fa3932497b06f9d0143100842354f62af`; settings comparisons require successful numeric parses and compare complete tables. |
| R6 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; history rows are checked by identity and order, and unbounded geometry cannot prove a fold. |
| R7 | fixed | `fbcbc82b1ebce1acf5e9144d974dec94077701aa`; history reruns require completed status and the expected transcript, and fail explicitly on failed status. |
| R8 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; meeting capture records the switch boundary and verifies post-switch system samples. |
| R9 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; live meeting events are collected during capture and require provisional delivery before stop. |
| R10 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`, `f000c13d4670c0fdbbaccee2dabb80b4137fa802`; CUA names preserve quoted text and the parser helpers are covered by focused tests. |
| R11 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; negative-text checks scan names and values and normalize only the exact profile span. |
| R12 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; Omarchy-bar checks use a widget-specific idle criterion and reject frozen active state. |
| R13 | fixed | `717ffa5b93a6b5ec9fa36e072393276854739d2f`; first-run Open config reads the daemon's `config` response field, with host regression coverage. |
| R14 | fixed | `da51ccfe7ae945f1859795a3b56b7a764cd86782`; app rendering uses the shared render-and-check path and planted style checks. |
| R15 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; strict approved-render comparison preserves size and checks colour distance, with canary coverage. |
| R16 | fixed | `2615d37ec74cd020125d9123a9264c2e8144f39e`; Home, first-run themes and concrete settings sections are represented in the visual manifest. History export/re-run/confirmation dialogs have no render switch and remain deliberately outside this manifest, as recorded in ADR 0052. |
| R17 | fixed | `84e6099a265b0972f681f288fca3eff4a46cf1c3`; install-session checks judge setup status and the actual desktop target. |
| R18 | fixed | `65b17f2ac3cb080913988c30b3bbd2a98ae48dcb`; the manifest enumerates plugin files so installed-list checks cannot omit the plugin. |
| R19 | fixed | `8559e472edd4dfed404077a1c2ac517adcca9d60`; ADR 0034 and 0004 point to the governing later records and the index is updated. |
| R20 | fixed | `f000c13d4670c0fdbbaccee2dabb80b4137fa802`; final local gate is green. |

### ADR

ADR 0052 records the QA isolation, strict visual evidence, MCP handshake, scenario assertion, manifest, packaging and supersession decisions for this theme.

### Verification

- `cargo check --workspace --all-targets`
- `cargo test -p dettivo-qa driver::cua --lib`
- `bash /home/gordon/work/dettivo-linux-wt/_factory/scripts/gate.sh /home/gordon/work/dettivo-linux-wt/fn-50 fn50-final`
- Final receipt: `GATE_fn50-final_EXIT=0 f000c13d4670c0fdbbaccee2dabb80b4137fa802`
- The gate passed workspace build and tests, 64/64 Qt smoke tests, rustfmt, clippy, dependency/file-length/delta/notice checks, packaging, QML, visual, settings, Omarchy, docs and rustdoc checks.

### Deliberately left out

No finding was rejected. History export, re-run and confirmation dialogs remain outside the visual matrix because they have no render switch; adding one is recorded as follow-up work for the History owner. Wave-3 themes, fn-38's human walkthrough and external release blockers remain outside fn-50.
## Evidence
- Commits: 728bfbbd7e2cc496951ab2600392864fe09d6710, e1ca8c22b8b662025378a0fac2410dcded5cd0b2, 2148572a681345e3d4191211fe4b2c2284d2c97b, 4a25e5e4fe74d88c32888b7690ec1d76d8ecbeba, 9cb025b96d5373b148023f216f2f59918cf4c5f8, fb752ed6ea566a8cb015d33ab5244ddd8c1d27b9, 01efbaf4e517c2d79c0f2a28ecedd29a1822d939, fae80bcc99ffe829d08fddbbea93249654604060, ecb48237d824e56ccef5e678b952f3697746afa3, 3040a8be49314b5d258329490102e15d0e9ea707, 7045c2a65cefee1da8bd1cba13efa4c9e3845dec, 56b27822554d33b71ad1006623fd62797ff5297b
- Tests: cargo check --workspace --all-targets, cargo test -p dettivo-qa driver::cua --lib, bash /home/gordon/work/dettivo-linux-wt/_factory/scripts/gate.sh /home/gordon/work/dettivo-linux-wt/fn-50 fn50-final
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
