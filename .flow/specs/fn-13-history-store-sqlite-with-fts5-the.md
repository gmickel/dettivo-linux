# History store: SQLite with FTS5, the transcripts API, re-run and export transfer

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-11 row: "`history-store-sqlite-fts-transcripts-api` | phase 1 | depends on S-07 | FR-Y1 to Y5, FR-S6 transfer for export | `transcripts.*` fixtures; FTS tests; export goldens"
> masterplan FR-Y1: "Dictation items store id, timestamps, engine, model, mode, language, duration, raw text, final text, insertion result, app id and optional audio path."
> masterplan FR-Y2: "`transcripts.list`, `get`, `latest`, `search`, `export` and `import` follow the contract, including `TranscriptRef` and `HistoryItem` shapes and statuses."
> masterplan FR-Y3: "Re-run transcribes the retained audio with a chosen engine, model or mode and stores a new version linked to the original."
> masterplan FR-Y4: "Full-text search covers dictation text, meeting segments, notes and analysis using SQLite FTS5. macOS uses lowercase substring matching over a denormalized search index; Linux keeps the same result shape, returns `score` as null while `knowledge.semantic_search` is false, and treats FTS5 as an internal upgrade."
> masterplan FR-Y5: "Delete removes the row and its artifacts according to the artifact policy."
> masterplan FR-S6: "`transfer.*` implements chunked upload and download for import and export with the contract limits and the macOS content types (`audio/wav`, `audio/mpeg`, `audio/mp4`, `audio/m4a`, `audio/aac`, `audio/x-caf`, `audio/caf`, `audio/aiff`, plus `audio/flac` and `audio/ogg` as Linux additions)."
> masterplan 8.3: "Database `$XDG_DATA_HOME/dettivo/dettivo.db` (SQLite, WAL, FTS5) ... Artifacts `$XDG_DATA_HOME/dettivo/dictations/<id>/` ... Entities: `dictations` (macOS `DictationItem` fields: id, created_at, app id and name, source kind `dictation|audioImport|rerun`, mode, stt provider and model, language, raw and final transcript, generated title and summary, duration_ms, rerun_of_item_id, error code and message, audio path, search index)"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Every dictation is kept and findable. `dettivo-storage` becomes the daemon's SQLite store (WAL, FTS5, migrations) for dictation items with the macOS `DictationItem` fields; S-07's session writes each finished dictation into it with its insertion result and optional retained audio; `transcripts.list`, `get`, `latest`, `search`, `export` and `import` answer per contract with the `TranscriptRef` and `HistoryItem` shapes; re-run transcribes retained audio again with another engine, model or mode as a new linked version; delete honours the artifact policy; and `transfer.*` provides the chunked download that `transcripts.export` hands out and the upload that `transcripts.import` consumes. `insert.perform` with a `source_ref` and `dictation.reinsert_last` read from the store, so re-insert survives a daemon restart. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `dettivo-storage` (depends on `dettivo-proto`, `dettivo-core`; `rusqlite` with the bundled SQLite and FTS5 feature): `Store::open(path)` sets WAL, foreign keys and busy timeout, runs numbered migrations from `migrations/`, and exposes typed repositories. Tables: `dictations` (id TEXT UUID, created_at, updated_at, app_id, app_name, source_kind `dictation|audioImport|rerun`, mode, stt_provider, stt_model, language, raw_text, final_text, title, summary, duration_ms, rerun_of_item_id, error_code, error_message, audio_path, insertion JSON), `dictations_fts` (FTS5 external content over raw_text, final_text, title, summary with the `unicode61` tokenizer and `remove_diacritics 2`, kept in sync by triggers), `schema_migrations`. Meeting tables arrive with S-23 through the same migration path. [paraphrase]
- Retention: `[history] keep_audio = true`, `audio_retention_days = 30`, `max_items = 0` (unlimited); a retained take is moved from the session's take file into `$XDG_DATA_HOME/dettivo/dictations/<id>/microphone.wav` with `metadata.json`; the daemon's hourly sweep deletes audio past retention and rows past `max_items`, oldest first. The artifact policy (`[history] artifacts = "keep" | "audio_only" | "none"`) decides what delete removes and what the sweep touches. [inferred]
- Session hook: S-07's pipeline ends by writing the item (raw text, final text, insertion result, app id from the target, duration, engine and model) before it publishes the final `dictation.state`; a storage error is logged and reported in the state payload's `error` but never loses the insertion. `dictation.reinsert_last` and `insert.perform { source_ref }` read from the store. [inferred]
- Search: `transcripts.search { query, scope?, limit?, cursor? }` runs an FTS5 `MATCH` with the query tokens ANDed and prefix-matched (`token*`) so the results equal macOS's substring behaviour for word starts; `score` is null (FR-Y4); results carry `HistoryItem` with snippet from `snippet()`. A query that FTS5 rejects (quotes, operators) is escaped as a phrase, never an error. [paraphrase]
- Re-run: `transcripts.rerun { id, provider?, model?, mode? }` (Linux addition until the contract carries it) starts a job that transcribes the retained audio through S-06's selection with the overrides, stores a new item with `source_kind = rerun` and `rerun_of_item_id`, and reports through `job.progress`; without retained audio it answers `NOT_FOUND` with `reason = audio_not_retained`. [paraphrase]
- Export: `transcripts.export { scope: item | range | all, format: json | markdown | txt | zip, ids?, from?, to? }` writes the export into `$XDG_CACHE_HOME/dettivo/exports/<transfer-id>` and answers a `TransferRef` that `transfer.pull` streams in chunks; `zip` bundles items with their audio. Formats are goldens. [paraphrase]
- Import: `transfer.begin` (content type from FR-S6's list, size within the contract limit), `transfer.chunk` (ordered, rate limited per the fixture's `RATE_LIMITED`), `transfer.commit`, then `transcripts.import { transfer_id, kind: audio | archive }` creates an `audioImport` item and a transcription job for audio, or restores items from a Dettivo zip; a second import while one runs is `CONFLICT` per the fixture. `transfer.cancel` drops the staging file. Staging lives in `$XDG_CACHE_HOME/dettivo/transfers/`. [paraphrase]
- QA seed: `DETTIVO_E2E_SEED=1` seeds twelve dictation items across three days and two apps (the macOS harness's expectation) at daemon start into the isolated data dir; the seed is deterministic for the drives and goldens. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `transcripts.list`, `get`, `latest`, `search`, `export`, `import` per the fixtures under `crates/dettivo-proto/fixtures/transcripts/`, including `export.error-invalid-format-scope` and `import.error-conflict-already-in-progress`. `transcripts.delete { id }` and `transcripts.rerun` are Linux additions recorded in `docs/api/linux-deltas.md` with fixtures. [paraphrase]
- `transfer.begin`, `chunk`, `commit`, `cancel`, `pull` per the fixtures under `fixtures/transfer/`, with `chunk.error-not-found` and `chunk.error-rate-limited`. [paraphrase]
- `dictation.reinsert_last` and `insert.perform { source_ref }` resolve against the store; `NOT_FOUND` for an unknown id. [paraphrase]
- CLI: `dettivo history list [--limit --app --since] | get <id> | latest | search <query> | delete <id> | rerun <id> [--model --provider --mode] | export --scope --format --out <file> | import <file>` (the CLI drives `transfer.*` itself for `export --out` and `import`), human tables and `--json`. [inferred]
- Config (`docs/config.md`): `[history] keep_audio`, `audio_retention_days`, `max_items`, `artifacts`, `db_path` (default per 8.3). `system.capabilities.history` reports `fts = true`, `semantic_search = false`. [inferred]
- Events: `job.progress` for re-run and import jobs; no new topic. [inferred]

## Edge Cases & Constraints

- Database locked by another process (a second daemon, a backup tool): the busy timeout waits five seconds, then the method answers `INTERNAL` with `reason = database_busy` and the daemon stays up. [inferred]
- Migration failure: the daemon refuses to start with the migration name and the backup path it wrote (`dettivo.db.bak-<version>`) before migrating. [inferred]
- Search query longer than 1 KB or with only stop characters: `INVALID_PARAMS` naming the limit. [inferred]
- Export of a range with no items: an empty export with the format's empty shape, not an error. [inferred]
- Import of an unsupported content type: `INVALID_PARAMS` listing the supported types (FR-S6 list plus the Linux additions). [paraphrase]
- Audio retention sweep during a re-run of that item: the sweep skips items with a running job. [inferred]
- Disk full while writing an item: the insertion still completes; the item is recorded without audio and the error is in the state payload. [inferred]

## Acceptance Criteria

- **R1:** Store unit tests: migrations from empty and from every previous version (a fixture database per version), WAL and foreign keys on, insert and read back of every `DictationItem` field, delete per artifact policy removing the row and the right files, the retention sweep, and concurrent readers during a write. Errors: a failed migration leaves the backup and names itself. [paraphrase]
- **R2:** FTS tests: word-start matches across raw text, final text, title and summary, diacritic-insensitive matching, phrase escaping of operator characters, snippets, paging with cursor, `score` null, and the twelve-item seed searchable by app and date. Errors: an over-long query is `INVALID_PARAMS`. [paraphrase]
- **R3:** Every `transcripts.*` and `transfer.*` fixture passes against the live daemon (contract test and the QA replay, with stateful fixtures covered by the daemon's integration tests by name); `transcripts.delete` and `transcripts.rerun` have fixtures and pass. Errors: the error fixtures for invalid format or scope, import conflict, chunk not found and rate limited pass. [paraphrase]
- **R4:** A dictation through the mock microphone ends with an item in the store carrying the transcript, insertion result, app id, engine and model, and retained audio when `keep_audio` is on; `dettivo history latest` shows it; after a daemon restart `dettivo dictation reinsert-last` inserts it again through the mock path. Errors: a storage failure is reported in the final state payload while the insertion stands. [paraphrase]
- **R5:** Re-run and export goldens: `dettivo history rerun <id> --model tiny.en` produces a linked `rerun` item from retained audio through the real Whisper engine and reports progress; `export --format json|markdown|txt` for the seed match checked-in golden files, `zip` contains the items and their audio and imports back into an empty profile with the same ids. Errors: re-run without retained audio is `NOT_FOUND` with `audio_not_retained`. [paraphrase]
- **R6:** `docs/history.md` describes the store, retention, search behaviour and the transfer flow; the `[history]` keys are documented and printed by `config print-default`; `dettivo doctor` reports the database path, size, item count and last migration; the CLI verbs are documented. Errors: as stated. [paraphrase]

## Boundaries

- No history GUI; S-20 reads this API and the seed. [paraphrase]
- No meeting tables, notes or analysis search; S-23 adds them through the same migrations and FTS. [paraphrase]
- No chunked long-audio pipeline for imports over the dictation limit; S-15 adds it, this spec transcribes an imported file as one take up to `[speech] max_import_seconds`. [inferred]
- No semantic search; `knowledge.*` stays `NOT_IMPLEMENTED`. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Agent surfaces (MCP, REST) and the history UI all read this store; landing it right after the session makes every later spec a consumer rather than a migration. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the `transcripts.*` and `transfer.*` methods answer the contract's fixtures, so MCP and the CLI expose history without translation. [strategy:Contract parity and agent surfaces]
- **Complete speech workflows, proven by drives:** a dictation is not complete until it is stored and re-insertable, and the drive proves both after a restart. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
