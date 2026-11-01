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

QA fixes (QA receipt qa-fn-83, all five P2s):
- QA-1: `dettivo config set dictation.vocabulary '[..., {"term": ...}]'` reads JSON when the text is no TOML value; the refusal names "a list of terms or { term, sounds_like } tables". Test: file_syntax config_set_writes_vocabulary_tables_and_keeps_comments_and_form (JSON-typed case).
- QA-2: `config set` keeps a hand-written multi-line list: surviving entries keep the comments above and beside them (including a comma-less last entry's comment), new entries get their own line. Test: file_syntax config_set_keeps_a_multi_line_list_and_its_entries_comments.
- QA-3: Settings counts a `config.changed` key as a file change only when its refreshed value or source differs from what the app held; `ConfigBinding::set` and `SettingsModel::unset` apply the daemon's answer, which reaches the app before the event. An older event's late refresh never sets the notice. Test: settings_pending_test ownWritesAreNotFileChanges.
- QA-4: a `config.changed` naming a key clears that key's earlier refusal. Same test.
- QA-5: the default file already described the table form and limits; the contract fixtures keys.json and print_default.json still pinned the old sentence (config_live keys_match_the_fixture failed against them). Both updated. The QA observation came from a daemon other than the worktree build (inferred: the worktree daemon prints the new sentence, measured).
- CHANGELOG Fixed lines for the multi-line list and the Settings notice/refusal.

Live check (isolated worktree daemon, no display/bus/Hyprland variables): JSON config set rc 0 with the multi-line file and comments kept; bad JSON alias refused with the new message (rc 4); `config keys` prints the new sentence.

Tests: vocabulary_schema (forms, misspelt key, dedup, each limit), validate (dedup and limit by key and line), file_syntax (config set round-trip in both forms and JSON, comments and multi-line layout kept), the Enhanced prompt format pin, session gate.rs (engine prompt carries terms, never aliases), tst_vocabulary_aliases.qml (show, edit, clear aliases, case duplicate), bench::vocabulary scoring (recall, insertions, per-clip guard WER), settings_pending_test ownWritesAreNotFileChanges.

Baseline: red (cargo test -p dettivo-qa failed pre-edit: dettivod not built in the fresh worktree; green once the workspace was built). QA-fix pass baseline: green.

Follow-ups (not part of this task):
- Partial prompt echo: with the vocabulary on, Whisper turned the distractor "Omar keeps the spare keys..." into `Omarchy, Hyprland.`. The echo guard (ADR 0081) drops only a whole-prompt echo, so this would paste. Recorded in ADR 0087.
- The bench does not measure Enhanced, so the aliases themselves are not scored yet.
- The ryan voice is CC BY-NC-SA 4.0, as in the existing diarization and meetings fixtures; set.json notes it.
- fn-80 also edits `dettivo-qa bench` (main.rs Bench options), so expect a merge conflict at integration.
- Settings route: Settings / Vocabulary rows gained a "Sounds like for <term>" field.

Decisions:
- An alias limit on length was not added; the spec bounds term length and alias count only.
- Review P2 (guard WER concatenated across clips) fixed after the first SHIP verdict (4498514); the re-review from 59199ff covers it.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran (codex gpt-6.1-sol high, base 59199ff: 3 draws NEEDS_WORK with 2 integration P2s, fixed in c507103, re-review SHIP)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 1df193d40759341e79da87bb823c23e881d83ed2, 2b3ad24b1fee9a486f873356fc298cc3711a8574, 9d0843136b77fd2052ff6490ca63ab7f57ec34a1, 59199ffce887c39b21915fe2e298b2975719a27a, 44985143e3a77e5314f6f158b92eb921c41107fd, faf7680092f3c8ebfe96c5b4720a3180ff47af25, c9fc713242ec33c48055bed41176efc60a651752, 25374873f32047573dd66bb9214b5ce9496b144f, 82bb22809f2c5eff36ee83953feffe21ddf0b949, c5071039ffb8a26cd0b55b1c0e06fc8b53b4b2b8
- Tests: cargo test -p dettivo-core -p dettivo-language -p dettivo-qa -p dettivo-session, cargo test -p dettivo-core -p dettivo-proto; cargo test -p dettivod --test config_live, ctest --test-dir build/qt offscreen: 86/86 incl. dettivo-quick-test (tst_vocabulary_aliases) and dettivo-settings-pending-test, just lint, isolated worktree daemon: dettivo config set (JSON vocabulary, multi-line layout, refusal) and config keys, dettivo-qa bench vocabulary --out docs/reports/benchmarks
- PRs: