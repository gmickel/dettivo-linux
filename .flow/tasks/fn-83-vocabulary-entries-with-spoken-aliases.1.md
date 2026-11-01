---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-83-vocabulary-entries-with-spoken-aliases.1 Implement Vocabulary entries with spoken aliases and a term-recall benchmark

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`[dictation] vocabulary` now takes plain terms and `{ term, sounds_like }` tables in one list. Limits are 500 entries, 64 characters a term and 16 aliases; a breach is refused with the entry named, and a term repeated in another case is dropped with the first spelling kept. The Enhanced prompt names each term with its aliases in the spec's format, Whisper and the echo guard still get terms only, and `config set` and Settings / Vocabulary edit both forms. `dettivo-qa bench vocabulary` scores 24 synthetic Piper clips; the baseline is filed in docs/reports/benchmarks/vocabulary-2026-11-01-thor.md (Whisper large-v3-turbo 2/15 off, 10/15 on, Parakeet v3 3/15). ADRs 0086 and 0087, docs and CHANGELOG are updated.

Tests: vocabulary_schema (forms, misspelt key, dedup, each limit), validate (dedup and limit by key and line), file_syntax (config set round-trip in both forms, comments kept), the Enhanced prompt format pin, session gate.rs (engine prompt carries terms, never aliases), tst_vocabulary_aliases.qml (show, edit, clear aliases, case duplicate), bench::vocabulary scoring (recall, insertions, per-clip guard WER).

Baseline: red (cargo test -p dettivo-qa failed pre-edit: dettivod not built in the fresh worktree; green once the workspace was built).

Follow-ups (not part of this task):
- Partial prompt echo: with the vocabulary on, Whisper turned the distractor "Omar keeps the spare keys..." into `Omarchy, Hyprland.`. The echo guard (ADR 0081) drops only a whole-prompt echo, so this would paste. Recorded in ADR 0087.
- The bench does not measure Enhanced, so the aliases themselves are not scored yet.
- The ryan voice is CC BY-NC-SA 4.0, as in the existing diarization and meetings fixtures; set.json notes it.
- fn-80 also edits `dettivo-qa bench` (main.rs Bench options), so expect a merge conflict at integration.
- Settings route: Settings / Vocabulary rows gained a "Sounds like for <term>" field.

Decisions:
- An alias limit on length was not added; the spec bounds term length and alias count only.
- Review P2 (guard WER concatenated across clips) fixed after the SHIP verdict, with a focused test, no re-review.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran (codex gpt-6.1-sol, 3 draws, SHIP)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 1df193d40759341e79da87bb823c23e881d83ed2, 2b3ad24b1fee9a486f873356fc298cc3711a8574, 9d0843136b77fd2052ff6490ca63ab7f57ec34a1, 59199ffce887c39b21915fe2e298b2975719a27a, 44985143e3a77e5314f6f158b92eb921c41107fd
- Tests: cargo test -p dettivo-core -p dettivo-language -p dettivo-qa -p dettivo-session, dettivo-quick-test -input tst_vocabulary_aliases.qml / tst_release_settings.qml (offscreen), scripts/qml-lint.sh build/qt; xtask lint-file-length/edges/deltas/notice; scripts/check-docs.sh; cargo clippy --workspace --all-targets -D warnings, dettivo-qa bench vocabulary --out docs/reports/benchmarks
- PRs: