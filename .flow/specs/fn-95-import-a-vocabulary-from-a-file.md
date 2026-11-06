# Import a vocabulary from a file

## Goal & Context

A vocabulary lives only in `config.toml`, `dettivo config set` and the Vocabulary editor, so someone with a list of 200 project names, colleagues and product terms enters them one by one.

This spec adds `dettivo vocabulary import <file>`, so a whole list goes in with one command and stays as editable as hand-written entries.

Origin: Gordon, 2026-11-06.

## Approach

- `dettivo vocabulary import <file>` in `dettivo-cli`: one term per line, blank lines and `#` comments ignored, and an optional `term = alias1, alias2` form for aliases. It supports what fn-83's entries allow and nothing more.
- It writes through the comment-preserving config edit path (`dettivo_core::config::edit`, ADR 0009). Existing entries are kept; duplicates are skipped and counted, compared the way the vocabulary already compares terms.
- The limits in `crates/dettivo-core/src/config/vocabulary_schema.rs` apply (500 entries, 64 characters a term, 16 aliases). An import that would break one writes nothing and names the failing lines.
- `--dry-run` prints what would change. The running daemon picks the change up the way it picks up any config edit.

## Acceptance

- **R1:** Importing a list adds its terms to `config.toml` with existing comments and entries preserved; a round-trip test covers it.
- **R2:** Duplicates are skipped and reported; a list that breaks a limit writes nothing and names the failing lines; `--dry-run` writes nothing.
- **R3:** The running daemon uses imported terms on the next take without a restart; a test covers it.
- **R4:** The term-recall bench (ADR 0087) gives the same result for a vocabulary imported from a file as for the same vocabulary written by hand.
- **R5:** The vocabulary doc describes the command and file format; a changelog entry is added.

## Boundaries

- Plain text lists only; no import from another application's database.
- No new vocabulary semantics; the import writes only entries the config already supports.

## Quick commands

- `cargo test -p dettivo-cli -p dettivo-core`
- `just build test lint`
