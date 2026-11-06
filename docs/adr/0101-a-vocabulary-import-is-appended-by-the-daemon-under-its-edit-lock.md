# 0101. A vocabulary import is appended by the daemon under its edit lock

Status: Accepted 2026-11-07

## What this gives you

`dettivo vocabulary import <file>` adds a list of 200 terms to `[dictation] vocabulary` in one command. Every entry and comment already in `config.toml` stays exactly as you wrote it, and an import never loses a term that Settings or another import added at the same moment.

## Situation

The first version of the command read the vocabulary in force with `config.get`, merged the new terms on the client, and wrote the whole list back with `config.set`. The implementation review on gpt-6.1-sol found two ways that lost data. Two imports, or an import and a Settings edit, could both read the same list and the second write replaced the first. And `config.get` returns the vocabulary after the loader drops a term repeated in another case, so writing it back deleted that repeat from the file with its comments, along with any comment inside an entry's `sounds_like` list.

The CLI is a client crate and talks to the daemon through `dettivo-proto` only, so it cannot run `dettivo_core::config::edit` itself.

## Decision

- The daemon gains `config.vocabulary.add { entries, dry_run }`. Under the config edit lock it reads the file text, skips each entry whose term the file or an earlier entry of the call already holds (compared in lower case, as the loader compares terms), checks the limits against the list as it would be written, and appends the new entries after the last entry of the file's own array (`dettivo_core::config::vocabulary_add`). A file without the key gets it through `edit::set`.
- An entry that breaks a limit refuses the whole call with `INVALID_PARAMS`. `details.refused` lists each such entry as `{ index, message }`, and nothing is written. The CLI maps each index back to the line of the import file.
- The result names the added entries and the skipped ones by index, each skip with the earlier entry it repeats, the number of entries after the call, and whether the file was written. A dry run and a call that adds nothing write nothing.
- Settings and `dettivo config set` keep writing the whole list with `config.set`.

## Consequences

- The entry count and the 256 KiB size budget count a repeat the file writes, as the loader's own limit check does, so an import is refused exactly where loading the written file would refuse it.
- A new entry takes the layout `config.set` gives it: its own line at the list's indent in a multi-line list, the same line in a one-line list.
- Without a running daemon the command exits 2 and writes nothing, as `dettivo config set` does.
