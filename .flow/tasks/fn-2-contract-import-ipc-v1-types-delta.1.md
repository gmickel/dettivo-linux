---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-2-contract-import-ipc-v1-types-delta.1 Implement Contract import: IPC v1 types, delta register and fixture suite

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Imported the macOS IPC v1 contract so a client written against Dettivo on macOS is checkable against Linux before any server exists: the IPC v1 and REST v1 documents copied unchanged with their commit pin (transcribe 6cb4e4a8, hashes in docs/api/CONTRACT_PINS.txt, drift fails the docs check); the dettivo-proto crate with the JSON-RPC envelope (string, integer and null ids, notifications without an id), every implemented method's params and result, the error taxonomy with numeric codes checked against app codes and an always-present details object, dotted event topics including the four Linux additions, the capability flags with the Linux platform and speech blocks, the shared runtime types, the reserved shapes, and a method catalog that runs any params or result through its typed shape by name; docs/api/linux-deltas.md registering every addition, behavioural difference and a disposition per contract item, enforced by the new xtask lint-deltas in just lint and CI; and a 72-file golden fixture suite (one directory per namespace, one file per method and error case, capability flags per file) that round-trips byte-stable in the Rust unit job, with its replay contract documented for the QA rig.

Reviewed and reworked the partial output of an earlier stopped run: kept the module layout and the runtime, capabilities, reserved and most method shapes after checking them against the contract; rewrote the envelope (typed ids), errors (typed app codes, code consistency, details as an object), events (one topic type in wire form, typed payloads), speech (Windows catalog shapes at e306e2c3) and polish (typed presets and mappings); added deny_unknown_fields everywhere, acknowledge_meeting_disclosure on meetings.start and transcripts.import, the catalog, the register, the lint, the fixtures and their tests.

Baseline: none (the spec defined no Quick commands before this task). Verify: just build test lint exit 0.

Coverage: R1 docs/api copies byte-identical to transcribe 6cb4e4a8 (cmp), pin in CONTRACT_PINS.txt and linux-deltas.md, scripts/check-docs.sh hash check; R2 tests/fixtures.rs round-trips all 72 fixtures byte-stable, unknown fields and non-UUID ids rejected with the field named through serde_path_to_error; R3 error.rs AppCode with rpc_code and ErrorData/JsonRpcError builders, mismatched or unknown codes rejected; R4 capabilities.rs platform and speech blocks, register names both, capability_flags_resolve_in_the_snapshot; R5 linux-deltas.md register table, xtask lint-deltas fails on a missing item or unknown disposition (unit tests); R6 reserved.rs plus catalog reserved set, one fixture per reserved method expecting NOT_IMPLEMENTED -32014; R7 fixtures run in cargo test on the unit-rust job, layout documented in fixtures/README.md and enforced by layout_names_every_namespace_and_method.

Notes for the conductor: RATE_LIMITED_LOCAL and APP_NOT_RUNNING carry no numeric code on macOS; Linux assigns -32016 and -32017 and registers both. The Windows provider catalog exposes display_name and model paths; the Linux shapes follow it exactly so agents can share code across ports.

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: 922ec5333190c485e6dbace80d3dc23477f49e91
- Tests: baseline: none (spec defined no Quick commands), just build test lint (exit 0; cargo build/test/fmt/clippy/doc, ctest 6/6, xtask lint-edges + lint-file-length + lint-deltas, scripts/qml-lint.sh, scripts/check-docs.sh), cargo test -p dettivo-proto (34 unit + 5 fixture tests; 72 fixtures round-trip byte-stable), cargo test -p xtask (14 tests incl. lint-deltas missing item, unknown disposition, duplicate row, missing marker), cmp docs/api/dettivo-ipc-v1.md and dettivo-rest-v1.md against ~/work/transcribe at 6cb4e4a8 (identical), cargo run -p xtask -- lint-deltas (exit 0, every contract item registered)
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
