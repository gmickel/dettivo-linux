---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-84-vocabulary-on-parakeet.1 Implement Vocabulary on Parakeet

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Parakeet now rescores its transcript towards `[dictation] vocabulary` (stage 1, ADR 0088): after the greedy decode, runs of one to four heard words that could be a term are scored by the TDT forward algorithm over the same frames from the same decoder state, as heard and as the term, and the term replaces them when `log P(term) + boost x pieces >= log P(heard)`. `[dictation] vocabulary_boost` (off, light, normal, strong; default normal) sets the boost, and the dictation request carries terms with their `sounds_like` aliases to Parakeet only.

- R1 met: off, an empty list or a take without a candidate is byte-identical to a plain decode (tests/vocabulary.rs).
- R2 MISSED at normal: term recall 3/15 -> 8/15 (5 of 12 missed terms recovered, target 8), 0 false insertions, guard WER 0.000 (docs/reports/benchmarks/vocabulary-2026-11-01-thor-rescoring.md). `strong` reaches 11/15 (8 of 12) with 0 false insertions. Raising normal's 2.5 nats or making strong the default is a product call left open.
- R3 met: rescoring adds a median 15.0 ms on clips with a candidate, 1.4 ms at most without, release build on a shared Vulkan GPU; per-clip numbers in the report.
- R4, R5 met with unit tests (bias/candidates.rs, bias/mod.rs).
- R6: stage 2 not built; ADR 0089 records why (it weighs the same transducer scores with the same boost, so it cannot close the 2.3-8.8 nat gaps measured).
- R7: docs/engines.md, docs/dictation.md, docs/config.md, docs/qa.md, CHANGELOG [Unreleased].

Decisions: Parakeet's `supports_custom_vocabulary` capability stays false (it describes a prompt); `dictation.vocabulary_boost` has no Settings control and is listed with a reason in settings_keys.cpp; file transcription, imports and meetings send no bias (dictation only). The new benchmark run is filed beside the fn-83 baseline instead of overwriting it.

Follow-ups: a Settings control for the boost level; the bias for Parakeet file transcription and imports; the full `just build test lint` gate (protocol and config schema changed) runs at the conductor/CI.

OPEN_ITEM: R2's two-thirds recall target is missed at `normal` (5 of 12); `strong` (4.0) meets it on the benchmark. Decide whether `normal` should add more than 2.5 nats per piece or the default level should be `strong`.

stage: impl-review - ran [codex gpt-6.1-sol: NEEDS_WORK (3 reviewers, 4 findings fixed) -> SHIP]
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: ca3bf927398b124a1fa0bd548dca10f7893cd579, 199eb1d51b84a22e36a8e8742edb7ea735166b94, 01abb275be38b2f1dbb7fb0a8fe4cde9734a7281, 2e1f0f4ccec702d0f81cc2cfa4232254012f2786, 14ceb779209938ea9be600b2ab6a48e247b90ef2
- Tests: cargo test -p dettivo-engine-parakeet -p parakeet-cpp-sys, cargo test -p dettivo-core -p dettivo-session -p dettivo-speech -p dettivo-engine-proto, cargo test -p dettivod --test config_live, cargo test -p dettivo-qa vocabulary, dettivo-qa bench vocabulary --engines target/release --out docs/reports/benchmarks
- PRs: