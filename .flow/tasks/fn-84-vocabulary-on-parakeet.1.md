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

Levels, per Gordon's decision of 2026-11-01 (R2 must be met at `normal`): `light` 2.5, `normal` 4.0, `strong` 5.0 nats per piece, measured on a term-recall set grown from 5 distractors and 4 guards to 25 and 7 (targets byte-unchanged; docs/reports/benchmarks/vocabulary-2026-11-01-thor-boosts.md).

| Boost | Term recall | Missed recovered | False insertions | Guard WER | Rescoring |
|---|---|---|---|---|---|
| off | 3/15 | 0 of 12 | 0 | 0.000 | - |
| 2.5 | 8/15 | 5 of 12 | 0 | 0.000 | 14.5 ms median, 1.0 ms most without |
| 4.0 | 11/15 | 8 of 12 | 0 | 0.000 | 14.4 ms median, 1.4 ms most without |
| 5.0 | 13/15 | 10 of 12 | 0 | 0.000 | 15.4 ms median, 1.2 ms most without |
| 6.0 | 14/15 | 11 of 12 | 1 | 0.000 | 14.2 ms median, 1.1 ms most without |

- R1 met: off, an empty list or a take without a candidate is byte-identical to a plain decode (tests/vocabulary.rs).
- R2 met at normal (4.0): 8 of 12 missed terms recovered, 0 false insertions on 25 distractors, guard WER 0.000. 6.0 wrote Hyprland over "hyper landing", so strong is 5.0; at 5.0 that distractor sits 3.2 nats short.
- R3 met: median 14-15 ms on clips with a candidate, at most 1.4 ms without, release Vulkan build on a GPU shared with other agents.
- R4, R5 met with unit tests (bias/candidates.rs, bias/mod.rs).
- R6: stage 2 not built; ADR 0089 records why and the new levels.
- R7: docs/engines.md, docs/dictation.md, docs/config.md, docs/qa.md, CHANGELOG [Unreleased].

Decisions: Parakeet's `supports_custom_vocabulary` capability stays false (it describes a prompt); `dictation.vocabulary_boost` has no Settings control and is listed with a reason in settings_keys.cpp; file transcription, imports and meetings send no bias (dictation only). Each boost was measured from its own engine build with `normal` mapped to it, since the bench runs its on pass at `normal`; the four raw runs are filed beside the report. make-vocabulary-fixture.py now renders only missing clips, because Piper synthesis is not byte-reproducible.

Follow-ups: a Settings control for the boost level; the bias for Parakeet file transcription and imports; the full `just build test lint` gate (protocol and config schema changed) runs at the conductor/CI. The set comes from two synthetic voices, so a real speaker's near-misses may sit closer than 0.8 nats per piece at strong.

stage: impl-review - ran [codex gpt-6.1-sol high: NEEDS_WORK -> SHIP at 14ceb77; boost change: SHIP (1 P3 count fix) -> re-review SHIP at 54945dc]
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: ca3bf927398b124a1fa0bd548dca10f7893cd579, 199eb1d51b84a22e36a8e8742edb7ea735166b94, 01abb275be38b2f1dbb7fb0a8fe4cde9734a7281, 2e1f0f4ccec702d0f81cc2cfa4232254012f2786, 14ceb779209938ea9be600b2ab6a48e247b90ef2, 3c9c02e5c087e879fd239de9f43c38129b543e23, 73bfa2cc7821213a12ebf45db5f292e58e13431f, 54945dcd32b6e705d10239a76a267b989be6dd6b
- Tests: cargo test -p dettivo-engine-proto -p dettivo-session -p dettivo-engine-parakeet -p dettivo-qa, cargo test -p dettivo-engine-parakeet -p parakeet-cpp-sys, cargo test -p dettivod --test config_live, just lint, dettivo-qa bench vocabulary, one engine build per boost 2.5/4.0/5.0/6.0
- PRs: