# Meeting notes, analysis, search, export, delete and the disclosure

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-26 row: "`meeting-notes-analysis-search-export-delete-disclosure` | phase 4 | depends on S-24, S-14 | FR-G6, G8, G9, `meetings.*`, `meetings.delete` | Fixtures; export goldens; delete artifact policy test"
> masterplan FR-G6: "Notes are editable Markdown separate from the transcript; analysis produces summary, decisions and action items through the LLM layer and can be regenerated."
> masterplan FR-G8: "Export produces `txt`, `md`, `srt`, `vtt` and `json` with speakers and timestamps; `md` includes notes and analysis when present; JSON keys are snake_case as on macOS (`id`, `title`, `started_at`, `ended_at`, `duration_ms`, `status`, `language`, `stt_provider_id`, `stt_model_id`, `transcript`, `segments[]`, `analysis`). Copy and export precedence is live editor, then saved notes, then generated analysis."
> masterplan FR-G9: "Before the first meeting the user acknowledges the disclosure notice, stored as `meeting.disclosure.acknowledged` with a timestamp. The message is the macOS text ... A "Copy Disclosure Message" action is available in the GUI and panel. `meetings.start` and `transcripts.import` for meetings accept `acknowledge_meeting_disclosure` so CLI, MCP and REST callers can satisfy the gate on a fresh profile."
> masterplan FR-Y4: "Full-text search covers dictation text, meeting segments, notes and analysis using SQLite FTS5."
> masterplan FR-Y5: "Delete removes the row and its artifacts according to the artifact policy."
> masterplan FR-M8: "The Enhanced and meeting-analysis models are the macOS catalogue in GGUF form: Qwen3 4B Instruct 2507 as the default for both ... Qwen3 8B for meeting analysis"
> masterplan 8.3: "`meetings/<id>/` with `microphone.wav` ..., `system.wav`, `transcript.json`, `notes.md`, `analysis.json`, `live-checkpoint.json`, `metadata.json`"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A meeting becomes a record, not a recording. Notes are Markdown the user writes during or after the meeting, stored beside the audio as `notes.md` and never touched by a model. Analysis is a summary, the decisions and the action items produced from the finalised transcript through the LLM provider layer the polish spec built and the local engine fills (`[llm] analysis_model`, the macOS analysis prompt ported verbatim), stored as `analysis.json`, regenerated on request, never overwriting notes. Search covers title, transcript, notes and analysis in one FTS index. Export renders the five formats with speakers and timestamps against goldens, `md` with notes and analysis, `json` with the macOS snake_case keys. `meetings.delete` applies the artifact policy exactly and is tested per policy. The disclosure gate gets its last pieces: the message as a golden, the copy action for GUI, panel and CLI, and the QA variable that lets a drive start on a fresh profile with or without the acknowledgement. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Notes (`crates/dettivo-storage`, `crates/dettivod/src/handlers/meetings_notes.rs`): migration `0006-notes-analysis` adds `notes_markdown`, `notes_source` (`user`, `live`), `notes_updated_at`, `analysis` (JSON), `analysis_status` (`none`, `running`, `ready`, `failed`), `analysis_error`, `analysis_model`, `analysis_at`, and `polished_text` on segments. `meetings.notes.set` writes the row and `notes.md` in the meeting directory atomically (the file is the copy a person opens; the row is what search and export read); `meetings.notes.get` returns both text and `updated_at`. Notes on a recording meeting are allowed (the live editor saves through the same call with `source = live`). [inferred]
- Analysis (`crates/dettivo-language`, `analysis` module; the job in `crates/dettivo-meeting/analysis.rs`): the macOS meeting analysis prompt and its JSON output contract ported as data with its golden cases (summary, decisions, action items with optional owner and due text) under `crates/dettivo-language/tests/goldens/meeting_analysis.json`; the transcript with speaker labels is chunked at `[meetings.analysis] chunk_chars` when it exceeds the model's context, each chunk summarised and the parts merged in one final pass (map then reduce), through the `LlmProvider` interface (`local` through the supervisor's `LlmEngine`, `ollama`, `openai_compatible` behind the trust gate, `DETTIVO_MOCK_LLM=echo|fixture:<dir>`), thinking blocks stripped, the output parsed strictly and a degenerate answer retried once then `failed` with the reason. It runs as a job after finalisation (and after the diarization pass when that spec's `auto` is on) when `[meetings.analysis] auto` is on, or on `meetings.analyze`. Analysis never writes notes. [paraphrase]
- Polished segments: at finalisation the deterministic Polish pass (`dettivo-language::polish`, mode `meeting`, the global transforms without app rules) fills `polished_text` per segment, so `transcript` is the polished join and `raw_text` the raw one; exports take `polished` unless `raw = true`. [inferred]
- Search: the meetings FTS index covers title, transcript, segment text, notes and analysis text; `meetings.search` and `transcripts.search` return the snippet from the matched column and name it in a Linux `matched_field` (`title`, `transcript`, `notes`, `analysis`); notes and analysis writes refresh the index in the same transaction. [inferred]
- Export (`crates/dettivo-storage/src/meeting_export.rs`): `txt` (time, speaker or side, text, gap markers), `md` (facts, the notes section per the precedence rule, the analysis section when present, the transcript with speakers), `srt` and `vtt` (cue per segment with the speaker prefix, `hh:mm:ss,mmm` and `hh:mm:ss.mmm`), `json` with the FR-G8 keys plus `notes`, `speakers` and `diarization` when present, `ended_at` and `duration_ms` from the row. Goldens under `crates/dettivo-storage/tests/goldens/meeting.{txt,md,srt,vtt,json}` rendered from the seed meeting. [paraphrase]
- Delete policy (`meetings_recovery::delete`, the retention module): `transcript_only` clears transcript, segments, speakers, analysis and the FTS row but keeps the row, the notes and the audio; `transcript_and_audio` additionally removes every WAV and the take sidecars and clears `audio_dir`, keeping the row's facts and notes; `all` removes the row and the directory. The active meeting is `CONFLICT` `sessionActive`; a running analysis or diarization job is cancelled first. This refines the capture spec's shipped behaviour and the ADR records it. [inferred]
- Seed: `DETTIVO_E2E_SEED=1` grows three meetings across two weeks (one completed with two named speakers, notes and ready analysis; one partial; one notes-only with no analysis) beside the contract's sample, so the fixtures, the goldens and the GUI drives share rows. [inferred]
- Disclosure: the macOS message is a golden in `crates/dettivo-proto/fixtures/meetings/disclosure.get.json` (already the fixture) and in the CLI snapshot; `dettivo meetings disclosure --copy` puts it on the clipboard through the insert crate's clipboard backend (`DETTIVO_MOCK_INSERT` writes it to the mock sink); `DETTIVO_E2E_DISCLOSURE=acknowledged|pending` seeds or clears the acknowledgement in the profile's `state.toml` under QA mode, so a drive can show the dialog on a fresh profile or skip it. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Linux additions with fixtures under `crates/dettivo-proto/fixtures/meetings/`: `meetings.notes.get { meeting_id }`, `meetings.notes.set { meeting_id, markdown, source? }` (`INVALID_PARAMS` over 1 MiB), `meetings.analyze { meeting_id, force? }` (a job; `CONFLICT` `analysisRunning`, `CONFLICT` `meetingNotCompleted`, `CONFLICT` `endpointNotTrusted`, the `notice` when no provider is available), `meetings.analysis.get { meeting_id }`. `meetings.start` and `transcripts.import { target_kind: "meeting" }` gain `analyze?`. [inferred]
- `meetings.get` and `transcripts.get { kind: meeting }` carry `notes`, `notes_source`, `analysis`, `analysis_status`, `ended_at`, `language`, `stt_provider_id`, `stt_model_id` and per segment `polished_text`; `meetings.list` items carry `summary`, `has_notes`, `analysis_status`, `is_partial`, `speaker_count`; `meetings.search` and `transcripts.search` carry `matched_field`. `transcripts.export { kind: meeting }` gains `notes_override?` (the live editor's text, first in the precedence) and `raw?`. `meeting.state` gains `analysis_status`. Registered in `docs/api/linux-deltas.md`. [inferred]
- Config: `[meetings.analysis] auto = true`, `timeout_ms = 60000`, `chunk_chars = 12000`, `provider = ""` (empty follows `[llm] provider`); `[llm] analysis_model` (exists); `[meetings] delete_artifact_policy = "all"` (the CLI and GUI default). [inferred]
- CLI: `dettivo meetings notes get|set <id> [--file <path>|--stdin]`, `dettivo meetings analyze <id> [--force]`, `dettivo meetings analysis <id>`, `dettivo meetings export <id> --format txt|md|srt|vtt|json --out <path> [--raw]`, `dettivo meetings delete <id> --policy transcript_only|transcript_and_audio|all`, `dettivo meetings disclosure --copy`. [inferred]
- MCP: `get_meeting` and the `meeting://{id}` resource carry notes and analysis; `search_meetings` carries `matched_field`; no new tools. [inferred]

## Edge Cases & Constraints

- Analysis on an empty transcript: `failed` with `emptyTranscript`, no model call. [inferred]
- Provider unavailable or over `timeout_ms`: `failed` with `provider_unavailable`; the transcript and notes are untouched; `meetings.analyze` later succeeds. [paraphrase]
- Regenerate with `force`: the previous analysis is replaced only when the new one parses; a failed regenerate keeps the old one and reports the error. [inferred]
- Notes saved while the meeting records: written to the row and file; the export precedence picks the live text when the client passes it. [paraphrase]
- Search hit inside notes: the snippet comes from the notes and `matched_field = notes`. [inferred]
- `transcript_only` on a meeting whose audio was already removed: succeeds, the audio facts stay absent. [inferred]
- A fresh profile under `DETTIVO_E2E_DISCLOSURE=pending`: `meetings.start` without the flag is `meetingDisclosureRequired` as before. [paraphrase]

## Acceptance Criteria

- **R1:** Notes: `meetings.notes.set` writes the row and `notes.md`, `meetings.notes.get` reads them back, the FTS index finds a word from the notes with `matched_field = notes`, and analysis never changes them (asserted after `meetings.analyze`). Errors: over-size notes are `INVALID_PARAMS` naming the limit. [paraphrase]
- **R2:** Analysis with `DETTIVO_MOCK_LLM=fixture:<dir>` produces summary, decisions and action items stored on the row and in `analysis.json`, the goldens under `crates/dettivo-language/tests/goldens/meeting_analysis.json` pass, a transcript over `chunk_chars` runs map then reduce, `force` regenerates, and a model-backed run with `llm/qwen3-1.7b` on this machine passes by name and skips without the model. Errors: provider unavailable or timeout is `failed` with `provider_unavailable`; a degenerate answer retries once then fails naming why. [paraphrase]
- **R3:** Export goldens: the seed meeting renders byte-equal to `crates/dettivo-storage/tests/goldens/meeting.{txt,md,srt,vtt,json}` (speakers, timestamps, gap markers, notes and analysis in `md`, the FR-G8 keys in `json`), `notes_override` takes precedence over saved notes and both over analysis in `md`, and `raw = true` renders the raw segments. Errors: an unknown format is `INVALID_PARAMS` listing the five. [paraphrase]
- **R4:** Delete per policy in `crates/dettivod/tests/meetings_delete.rs`: `transcript_only` keeps row, notes and audio and clears texts, segments, speakers, analysis and the FTS row; `transcript_and_audio` removes the WAVs and sidecars and keeps the facts and notes; `all` removes row and directory; the active meeting is `CONFLICT`. Errors: as stated. [paraphrase]
- **R5:** Every `meetings.*` fixture, including the additions and the seed-backed `list` and `search` shapes, passes against the live daemon; the replay reports no pending meeting method; `system.capabilities.meetings.methods` lists them; the CLI `--json` snapshots cover the new verbs; `dettivo meetings disclosure --copy` lands the macOS text in the mock sink and `DETTIVO_E2E_DISCLOSURE` seeds or clears the acknowledgement. Errors: as stated. [paraphrase]
- **R6:** `docs/meetings.md` gains notes, analysis, search, export and delete sections with the precedence rule and the disclosure copy; the config keys are documented and printed by `config print-default`; the CLI verbs and QA variable are documented in `docs/qa.md`; an ADR records the analysis path, the polished segments and the delete policy. Errors: as stated. [paraphrase]

## Boundaries

- No speaker assignment or renames (S-25); exports render whatever `speaker` text the row carries. [paraphrase]
- No GUI: the notes editor, analysis rail, export sheet and disclosure dialog are S-27. [paraphrase]
- No meeting templates, retention auto-delete, email delivery or knowledge Q&A (reserved in the contract). [paraphrase]
- No analysis of dictations. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Notes the user owns, an analysis the model owns and a search that finds both is the recall the strategy promises; goldens and a per-policy delete test make it safe to trust with real meetings. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the five export formats, the FR-G8 JSON keys and the disclosure flag match macOS and Windows; every method has a fixture. [strategy:Contract parity and agent surfaces]
- **Complete speech workflows, proven by drives:** notes, analysis, search, export and delete close the meeting loop with goldens and daemon tests. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
