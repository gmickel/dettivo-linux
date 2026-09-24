# 0014. One SQLite file with FTS5 is the history store; artifacts sit beside it under a policy

Status: Accepted 2026-09-04

## What this gives you

Every dictation you finish is stored before the daemon says it is idle, and any client (the command line today, the app, the MCP server and the REST shim later) reads the same rows through `transcripts.*`. Search finds word starts across raw text, final text, title and summary with accents folded, a re-run keeps the original and links the new take to it, an export is a file you can open anywhere, and a backup is one file plus one directory.

## Situation

The macOS port keeps `DictationItem` rows in a SQLite database with a denormalised lowercase index for substring search; the Windows port stores settings and history in SQLite too. The Linux daemon needed a store before the history screen, the agent surfaces and the meetings spec could be built, and the contract already fixes the shapes it must answer with (`HistoryItem`, `TranscriptRef`, `transcripts.search` with a nullable `score`). Two questions were open: whether search should be an application-side index like macOS or the database's own full-text engine, and where retained audio and sidecars live so that delete, retention and export agree.

## Decision

`crates/dettivo-storage` owns one SQLite database, `$XDG_DATA_HOME/dettivo/dettivo.db` (`[history] db_path` moves it), opened in WAL mode with foreign keys on and a five second busy timeout, through `rusqlite` with the bundled SQLite so the feature set does not depend on the distribution's library. Numbered SQL migrations are compiled into the daemon; before the first pending migration touches an existing database the daemon writes `dettivo.db.bak-<version>` with `VACUUM INTO`, and a migration that fails stops the daemon at start naming itself and that file.

Search is SQLite FTS5 over an external-content table kept in step by triggers, tokenised with `unicode61` and `remove_diacritics 2`. A query becomes quoted prefix terms ANDed together, so the results equal the macOS substring behaviour for word starts and operator characters are never syntax. `score` stays `null` and `system.capabilities.history` says `fts = true`, `semantic_search = false`; FTS5 is an internal upgrade, not a contract change.

Retained audio and sidecars live beside the database under `$XDG_DATA_HOME/dettivo/dictations/<id>/`, and `[history] artifacts` names what is written and what a delete removes: `keep` (audio and `metadata.json`, delete removes the directory), `audio_only`, or `none`. `[history] keep_audio` replaces `[audio] keep_dictation_audio`: the take belongs to the item, not to the capture. The hourly sweep applies `audio_retention_days` and `max_items` and skips items a job is using.

Linux adds three methods, `transcripts.delete`, `transcripts.rerun` and `transcripts.stats`, plus `scope`, `from`, `to` and the `zip` format on `transcripts.export` and `app_id`, `since`, `until` on `transcripts.list`, all optional and all registered in `docs/api/linux-deltas.md`. Meeting kinds answer `NOT_IMPLEMENTED` until the meetings spec adds its tables through the same migrations.

## Consequences

- A dictation is stored before its final `dictation.state`; a store that fails reports on that state's `reason` and never undoes the insertion. The busy timeout bounds the wait at five seconds, after which the method answers `INTERNAL_ERROR` with `reason = database_busy`.
- The archive export is a stored (uncompressed) ZIP written by the crate itself, so no compression library enters the daemon and any unzip tool opens it; audio is already compact and `items.json` is small.
- Imports of formats other than WAV ran through `ffmpeg` at first; ADR 0022 replaced that with an in-process Symphonia decoder and the chunked pipeline.
- The `transcripts.*` fixtures that address a meeting stay pending in the contract replay until the meetings spec lands; the replay's daemon carries the twelve-item seed so the dictation fixtures answer.
- A `config.toml` that still sets `[audio] keep_dictation_audio` fails validation and names the key; `dettivo config validate` shows it and the daemon runs on defaults until it is moved to `[history] keep_audio`.
