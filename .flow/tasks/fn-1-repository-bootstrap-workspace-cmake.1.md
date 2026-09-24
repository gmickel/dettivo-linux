---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-1-repository-bootstrap-workspace-cmake.1 Implement Repository bootstrap: workspace, CMake skeleton and CI

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Bootstrapped the repository so one command builds, tests and lints every Rust crate and every Qt binary the same way CI does: a 16-crate Cargo workspace plus an `xtask` lint tool (crate dependency edges from a declared table, file-length limits with an allowlist), a CMake project for `dettivo-app`, `dettivo-osd` and `dettivo-insert-target` with the shared `Dettivo` QML module installed to `/usr/share/dettivo/qml` behind a Qt 6.8 floor that names the found version, a `justfile` (mirrored by a `Makefile`) whose recipes fan out through `scripts/run-step.sh` so the first failing toolchain is named, a digest-pinned Arch Linux CI with jobs `fmt`, `clippy`, `lint`, `unit-rust`, `unit-qml`, `docs`, a value-first CONTRIBUTING guide, a README, and the feature map index seeded from the design coverage table.

Baseline: none (the spec defined no Quick commands before this task). Verify: `just build test lint qa` exit 0.

Coverage: R1 justfile+Makefile+run-step+check-toolchain (missing dependency named, build stops); R2 crates/* + tools/xtask edges (red-checked with a forbidden dettivo-cli -> dettivo-core edge); R3 qt/CMakeLists.txt floor + install rules (installed tree verified with a temp prefix and an external `import Dettivo` consumer); R4 .github/workflows/ci.yml + composite action; R5 pipeline.qa=off, review.backend=null already in .flow/config.json, .flow/features/README.md seeded; R6 CONTRIBUTING.md; R7 xtask lint-file-length (red-checked with a 501-line file) + .file-length-allow.

Notes for the conductor: `just` is not installed on this machine and sudo needs a password, so verification used a scratchpad copy of just 1.58.0; the Makefile mirror covers a machine without it. Vulkan headers are absent here; `scripts/check-toolchain.sh --engines` reports them and CI runs that flag, while `just build` checks only what the bootstrap crates need (no crate links ggml yet) - the engine specs turn the flag on for `build`. The feature map holds one index entry per design surface under "Identified, not yet proven" and no feature files, because the features contract forbids a file for an undriven route. /usr/bin/qmllint on Arch is the Qt 5 tool; scripts/qml-lint.sh resolves the Qt 6 tools via `qmake6 -query QT_HOST_BINS`. The Dettivo module ships as one shared plugin library under /usr/share/dettivo/qml/Dettivo (namcap will flag a .so under share; the packaging spec can move to NO_PLUGIN if preferred).

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: 284a402e9dea31c5d7d756cde6901ab2314512e9
- Tests: baseline: none (spec defined no Quick commands), just build test lint qa (exit 0; cargo build/test/fmt/clippy/doc, ctest 6/6, xtask lint-edges + lint-file-length, scripts/qml-lint.sh, scripts/check-docs.sh), red-check: cargo run -p xtask -- lint-edges fails on dettivo-cli -> dettivo-core (exit 1), red-check: cargo run -p xtask -- lint-file-length fails on a 501-line .rs (exit 1), cmake --install to a temp prefix: share/dettivo/qml/Dettivo/{qmldir,Theme.qml,Dettivo.qmltypes,libDettivo.so}, scripts/check-toolchain.sh --engines exit 1 naming vulkan-headers (absent on this host)
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
