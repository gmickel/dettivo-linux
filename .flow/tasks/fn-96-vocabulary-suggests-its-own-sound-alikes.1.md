---
satisfies: [R1, R2, R3, R4]
---
# fn-96-vocabulary-suggests-its-own-sound-alikes.1 Implement Vocabulary suggests its own sound-alikes

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`dettivo vocabulary suggest [term...] [--apply]` (daemon method `speech.vocabulary.suggest`, ADR 0104): two Piper voices say each term alone and in "I think {term} is great."; the selected engine transcribes each clip with the vocabulary in force and with none. Terms all clips get right are flagged; for clips still wrong, the unprimed words where the term was are proposed as sounds_like. `--apply` writes through `config.vocabulary.add` with the new `extend` option (held entries gain spellings in place). `dettivo-qa bench vocabulary --vocabulary <file>` measures a generated vocabulary.

Evidence: R1 live runs on the five ADR 0087 terms, byte-identical across fresh daemons for Parakeet v3 and Whisper large-v3-turbo. R2 bench: Parakeet 13/15 and Whisper 10/15 with generated spellings, same as terms only, 0 false insertions; Whisper single names 6/10 to 7/10 (docs/reports/benchmarks/vocabulary-2026-11-08-thor-*). R3: crates/dettivo-cli/tests/vocabulary_suggest.rs (mutation-checked). R4: docs/dictation.md, ADR 0104, changelog.

stage: impl-review - ran (model: codex:gpt-6.1-sol:high) SHIP; four P2 findings fixed in 5a41a05 with focused tests

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 2657bb864c1ef08b0088866695cc4966626d8ef8, 5a41a05e50fc35049fa9a0eaeb2b61f41a614746
- Tests: cargo test -p dettivo-cli -p dettivo-qa, cargo test -p dettivo-core vocabulary_add, cargo test -p dettivo-speech suggest, cargo test -p dettivo-proto, dettivo-qa bench vocabulary --vocabulary (terms-only, suggested-parakeet, suggested-whisper)
- PRs: