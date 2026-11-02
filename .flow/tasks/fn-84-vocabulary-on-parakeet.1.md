---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-84-vocabulary-on-parakeet.1 Implement Vocabulary on Parakeet

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Parakeet now rescores its transcript towards `[dictation] vocabulary` (stage 1, ADR 0088). After the greedy decode, runs of one to four heard words that could be a term are scored by the TDT forward algorithm over the same frames from the same decoder state, once as heard and once as the term. The term replaces the heard words when `log P(term) + boost x pieces >= log P(heard)`. `[dictation] vocabulary_boost` (off, light, normal, strong; default normal) sets the boost, and the dictation request carries terms with their `sounds_like` aliases to Parakeet only.

Levels, per Gordon's decision of 2026-11-02 (goal fn-88 log, best-in-class bar): `light` 4.0, `normal` 5.0, `strong` 6.0 nats per piece (`ec31f13`). They are measured on clips padded the way dictation pads a take (bench padding `ad49316`, docs/reports/benchmarks/vocabulary-2026-11-01-thor-padded.md).

| Level | Boost | Term recall | Missed recovered | False insertions | Guard WER | Rescoring |
|---|---|---|---|---|---|---|
| off | 0 | 3/15 | 0 of 12 | 0 | 0.000 | - |
| light | 4.0 | 10/15 | 7 of 12 | 0 | 0.000 | 14.5 ms median, 1.3 ms most without |
| normal (shipped build, no remap) | 5.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.3 ms median, 1.3 ms most without |
| strong | 6.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.8 ms median, 1.1 ms most without |

The `normal` row comes from a normal engine build at `ec31f13` (vocabulary-2026-11-01-thor-padded-normal.json) and matches the remapped 5.0 run clip for clip. The bench always runs its `on` pass at `normal`, so the light and strong rows come from the padded report's remapped 4.0 and 6.0 builds. Margins: normal is 0.57 nats per piece above the last Hyprland clip and 1.41 below the "hyper landing" distractor; strong is 0.41 from it. All of these are measured on two synthetic voices.

QA fix (S5-possessive-rewritten-at-strong, P2, `5c44849`): Parakeet hears "Hyprland's" as "Hyperlans", which is not the term or an alias plus `s`, so at `strong` 6.0 the term won the span (margin +1.42 padded) and wrote "Hyprland config is short.". A run that would be a candidate once its trailing `s` or `es` is dropped is now never one, when no spelling of the term ends in `s`. The skip covers that run only, so "Hyperland draws" still leaves "Hyperland" a candidate; a first version that put it in the overlap-blocking set lost target-hyprland-1 (12/15) and was reworked. After: the possessive clip gives "Hyperlans config is short." at strong and normal, padded and raw, with no candidate; the plural stays "Hyperlands". Padded bench at normal: 13/15, 0 false insertions, guard WER 0.000, transcripts identical clip for clip to the shipped normal run. Tests: `an_inflected_mishearing_is_never_a_candidate` (bias/candidates.rs) covers the fused possessive and plural, the `Kubernetes` exception and the "Hyperland draws" regression. ADR 0088 lists the rule.

- R1 met: off, an empty list or a take without a candidate is byte-identical to a plain decode (tests/vocabulary.rs).
- R2 met at normal (5.0): 10 of 12 missed terms recovered, 0 false insertions on 25 distractors, guard WER 0.000, on padded clips. The two misses are `Itivo` (needs 6.04) and `DeDivo` (7.03) for Dettivo.
- R3 met: 14-15 ms median on clips with a candidate, at most 1.3 ms without, release Vulkan build, GPU shared with other agents (20-40 % busy).
- R4, R5 met with unit tests (bias/candidates.rs, bias/mod.rs).
- R6: stage 2 not built; ADR 0089 records why, with the padded levels.
- R7: docs/engines.md, docs/dictation.md, docs/config.md, docs/qa.md, CHANGELOG [Unreleased] at 4.0/5.0/6.0 and the padded numbers.

Decisions: Parakeet's `supports_custom_vocabulary` capability stays false (it describes a prompt). `dictation.vocabulary_boost` has no Settings control and is listed with a reason in settings_keys.cpp. File transcription, imports and meetings send no bias (dictation only). ADR 0089 keeps its historical table of the spec-proposed levels (2.0/2.5/4.0, unpadded) as the situation that triggered the decision.

Follow-ups: when a padded take comes back empty, the bench's retry keeps wall time but drops the first attempt's rescoring time (vocabulary_run.rs:211, review P2; no filed number is affected: no filed run has an empty transcript, and an empty take has no candidate). A Settings control for the boost level. The bias for Parakeet file transcription and imports. The shipped-level run also measured Whisper large-v3-turbo on the CPU (on: 10/15, 3 insertions), which earlier runs skipped.

stage: impl-review - ran [codex gpt-6.1-sol high: NEEDS_WORK -> SHIP at 14ceb77; boost change: SHIP at 54945dc; padding + levels 4.0/5.0/6.0: SHIP (3 lenses, 1 P2 follow-up) at 9e27d49; QA possessive fix: SHIP (1 reviewer, gpt-6.1-sol high) at 5c44849]
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: ca3bf927398b124a1fa0bd548dca10f7893cd579, 199eb1d51b84a22e36a8e8742edb7ea735166b94, 01abb275be38b2f1dbb7fb0a8fe4cde9734a7281, 2e1f0f4ccec702d0f81cc2cfa4232254012f2786, 14ceb779209938ea9be600b2ab6a48e247b90ef2, 3c9c02e5c087e879fd239de9f43c38129b543e23, 73bfa2cc7821213a12ebf45db5f292e58e13431f, 54945dcd32b6e705d10239a76a267b989be6dd6b, ad49316632eb085c035c4b5231e23d3a7bdd27e9, 052946b523b1567e0555292c0c84c2c7edf30463, ec31f13f1bb4922d577173ae2f08c1d7ff0ed4d0, 9e27d49b2148681d4d5d13f1d81481e5941005f2, 5c448491ec139e5a4e7f0f1440ba19328c7dd397
- Tests: cargo test -p dettivo-engine-proto -p dettivo-session -p dettivo-engine-parakeet -p dettivo-qa, cargo test -p dettivo-engine-parakeet -p parakeet-cpp-sys, cargo test -p dettivod --test config_live, cargo test -p dettivo-session -p dettivo-engine-parakeet -p dettivo-qa -p dettivo-transcribe (296 passed), just lint, dettivo-qa bench vocabulary --engines target/release (release Vulkan build at ec31f13, shipped normal 5.0: 13/15, 0 false insertions, guard WER 0.000), dettivo-qa bench vocabulary padded, one engine build per boost 2.5/4.0/5.0/6.0, cargo test -p dettivo-session -p dettivo-engine-parakeet -p dettivo-qa (265 passed), just lint, dettivo-qa bench vocabulary --engines target/release (release Vulkan build at 5c44849, normal 5.0 padded: 13/15, 0 false insertions, guard WER 0.000, transcripts identical to vocabulary-2026-11-01-thor-padded-normal.json), dettivo-engine-parakeet --wav r5-possessive.wav --boost strong (padded and raw: Hyperlans config is short., candidates=0)
- PRs: