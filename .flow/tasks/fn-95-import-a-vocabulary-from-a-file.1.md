---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-95-import-a-vocabulary-from-a-file.1 Implement Import a vocabulary from a file

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`dettivo vocabulary import <file> [--dry-run]` adds a plain-text list (one term per line, `term = spelling, spelling` for sounds_like, blank and `#` lines skipped) to `[dictation] vocabulary`. The daemon method `config.vocabulary.add` does the merge under the config edit lock against the file text (`dettivo_core::config::vocabulary_add`): it skips terms the file or an earlier entry holds (lower-case compare, as the loader dedups), checks every limit against the list as written, appends after the file's last entry so existing entries and comments stay byte-for-byte, and refuses the whole call with `details.refused` by index, which the CLI maps to file lines. Docs: docs/dictation.md "Importing a list", ADR 0101, linux-deltas rows, agent guide CLI tree, changelog.

- R1: `an_import_adds_the_terms_and_keeps_the_files_comments_and_entries`, `an_import_keeps_a_written_repeat_and_the_comments_inside_an_entry`, `imports_at_the_same_time_each_keep_their_terms` (crates/dettivo-cli/tests/vocabulary_import.rs); core `new_entries_follow_the_last_and_every_written_entry_keeps_its_text`.
- R2: `duplicates_are_reported_and_a_broken_limit_or_a_dry_run_writes_nothing`; core `every_entry_that_breaks_a_limit_is_refused_by_index`, `the_byte_budget_is_the_serialized_list_the_loader_checks`, `a_term_the_file_or_an_earlier_entry_holds_is_skipped_in_any_case`.
- R3: `the_running_daemon_uses_imported_terms_on_the_next_take` (temp tree, test daemon, Parakeet v2 + jfk.wav: "country" before, "Kuntry" after the import, same pid; skipped by name when the model is absent).
- R4: `the_bench_vocabulary_imported_from_a_file_is_the_one_written_by_hand` (fixture tests/fixtures/vocabulary.txt imports to exactly set.json's entries and equals a hand-written config's vocabulary; the bench primes engines from those entries alone).
- R5: docs/dictation.md#importing-a-list, CHANGELOG.md Unreleased/Added.

stage: impl-review - ran [round 1 NEEDS_WORK (2 findings: lost update across RPCs; config.get write-back dropped a file repeat and nested comments) -> fixed in 0f0a6da -> round 2 SHIP] (model: gpt-6.1-sol high via codex)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: e9e8237e0027510a59d28d57dbbc730ff19f9faa, 0f0a6daafb6ed12016f0ad75c4e6d527dfe86d76
- Tests: cargo test -p dettivo-cli -p dettivo-core -p dettivo-proto, cargo test -p dettivod --bins --test contract --test config_live --test smoke, cargo test -p dettivo-qa --lib, cargo test -p dettivo-mcp -p dettivo-rest
- PRs: