# Audio import: decoding, the chunked long-audio pipeline and the overlap merger

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-15 row: "`audio-import-transfer-chunked-long-audio` | phase 2 | depends on S-11 | `transcripts.import`, `transfer.*`, chunked pipeline (shared with meetings) | Transfer fixtures; chunked merge unit tests with overlap fixtures"
> masterplan FR-S6: "`transfer.*` implements chunked upload and download for import and export with the contract limits and the macOS content types (`audio/wav`, `audio/mpeg`, `audio/mp4`, `audio/m4a`, `audio/aac`, `audio/x-caf`, `audio/caf`, `audio/aiff`, plus `audio/flac` and `audio/ogg` as Linux additions)."
> masterplan FR-G2: "The offline path for finalization and import chunks at 300 s with 2 s overlap and a 5 s safety margin (the macOS code values; the macOS prose saying 10 s is stale). Both require a timestamp-capable engine, merge overlap by timestamp and interleave sources on absolute meeting time."
> masterplan FR-G3: "Known filler hallucinations are filtered before labels and analytics, and near-silent audio is guarded so Whisper does not invent text."
> masterplan 5.1: "Chunked timestamped transcription, overlap merge, dual-source interleave, live transcript events, hallucination filter."
> masterplan S-24 row: "`meeting-transcription-chunked-merge-live-events` | phase 4 | depends on S-23, S-15"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

An hour of audio in any of the contract's formats becomes a transcript. The history spec landed `transfer.*` and `transcripts.import` for a single take up to the dictation limit; this spec makes import real for long audio and lays the offline half of the meeting pipeline: `dettivo-audio` decodes every FR-S6 content type to 16 kHz mono through Symphonia, the new `dettivo-transcribe` crate chunks at the macOS values (300 s windows, 2 s overlap, 5 s safety margin), transcribes each chunk through the selected timestamp-capable engine, merges the overlaps by timestamp, filters the known filler hallucinations and guards near-silent chunks, and reports progress per chunk on `job.progress`. The item lands in the history with segments and word timestamps, and the meeting spec later reuses the same merger for two sources on absolute time. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Decoding (`crates/dettivo-audio/src/decode.rs`): `symphonia` with the `wav`, `aiff`, `caf`, `isomp4`, `aac`, `mp3`, `flac`, `ogg` and `vorbis` features decodes any FR-S6 type to interleaved f32, downmixes to mono and resamples to 16 kHz (`rubato` sinc), streaming in blocks so a one-hour file never sits in memory whole; the content type from `transfer.begin` selects the probe hint and the decoded duration, sample rate and channel count are recorded on the item. A file whose container does not match its declared type is refused naming both. [inferred]
- `crates/dettivo-transcribe` (depends on `dettivo-proto`, `dettivo-speech`, `dettivo-language`): `chunker` (fixed 300 s windows with 2 s overlap and a 5 s safety margin cut at the quietest point inside the margin, so a chunk never ends mid-word when it can help it), `merger` (segments and words from consecutive chunks merged by timestamp inside the overlap: the later chunk's words win from the overlap midpoint, duplicated words within 250 ms are dropped, segment boundaries snap to the merged words), `filters` (the known filler hallucination list from macOS, applied before labels; a near-silence guard per chunk with the RMS floor from macOS that skips transcription and emits no text), `job` (drives chunks through the engine with `recognize`, publishes `job.progress` per chunk with `chunks_done / chunks_total`, cancellable between chunks). The live windowed path of FR-G2 is out of scope here; the merger is written so both paths share it. [paraphrase]
- Import wiring: `transcripts.import` for `audioImport` items now decodes and runs the chunked job instead of the single-take path, stores `duration_ms`, segments and words on the item (the history spec's schema gains a `segments` JSON column through a migration), keeps the decoded 16 kHz WAV as the retained audio when `[history] keep_audio` is on, and answers `NOT_FOUND`/`INVALID_PARAMS` per the fixtures. Re-run of an imported item goes through the same job. [inferred]
- CLI: `dettivo history import <file> [--provider --model --language]` streams the file through `transfer.*` in `chunk_max_bytes` pieces and follows `job.progress` with a progress line; `dettivo history get <id> --words` prints the words with timestamps. [inferred]
- Limits: `[history] max_import_seconds = 14400`, `[transfer] max_upload_bytes` (the existing 1 GiB), documented; a file over the duration limit is refused after the probe, before decoding. [inferred]
- MCP `import_audio` gains its live harness step now that the import can run without a real microphone (fixture WAV, mock engine). [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `transcripts.import { transfer_id, target_kind: "dictation", filename, content_type, provider?, model?, language? }` per the fixtures; `job.progress` payload gains `chunks_done`, `chunks_total` and `stage` (`decoding`, `transcribing`, `merging`) as a Linux addition in the delta register. [inferred]
- `transcripts.get` returns `segments` with `words` for imported and re-run items (the contract's `HistoryItem` shape already reserves segments; the delta register notes when Linux fills them). [inferred]
- CLI verbs above; `dettivo-qa pipeline import-merge` (L2) runs the overlap fixtures through the engine CLI modes and the merger and compares with goldens. [inferred]
- Config: `[history] max_import_seconds`, `[transcribe] chunk_seconds = 300`, `overlap_seconds = 2`, `safety_margin_seconds = 5`, `silence_rms_floor`, `filler_filter = true`. [inferred]

## Edge Cases & Constraints

- A file shorter than one chunk: one chunk, no merge, same code path. [inferred]
- Overlap with no words on either side: the merge is a no-op and the boundary snaps to the chunk cut. [inferred]
- The engine crashes on chunk 7 of 12: the supervisor restarts it, the job retries the chunk once, then fails the item with the chunk index in the error; earlier chunks are kept. [inferred]
- Cancel mid-job: the item is marked cancelled, the decoded WAV is removed unless retained, progress reports the stop. [inferred]
- A silent hour: every chunk is guarded, the item stores an empty transcript with `notice = silent`, never invented text. [paraphrase]
- Variable-bitrate MP3 with a wrong duration header: the decoder's true length wins; the probe's estimate only gates the limit. [inferred]

## Acceptance Criteria

- **R1:** Decoding unit tests convert a short fixture in every FR-S6 content type (WAV, AIFF, CAF, M4A, AAC, MP3, FLAC, OGG; fixtures generated once and checked in) to 16 kHz mono with the expected sample count within tolerance, streaming in blocks; a mismatched container is refused naming the declared and detected types. Errors: as stated. [paraphrase]
- **R2:** Chunker and merger unit tests with overlap fixtures: the macOS window, overlap and margin values, the quiet-point cut, duplicate words dropped across the overlap, boundaries snapped, segment order preserved; goldens for a three-chunk synthetic case. Errors: a chunk with no timestamps (a non-timestamp engine) fails the job naming the engine's capability. [paraphrase]
- **R3:** Filters: the filler hallucination fixture list is removed before labels, the near-silence guard skips a silent chunk and emits no text, and a mixed fixture keeps its speech; both are unit-tested with the macOS values. Errors: as stated. [paraphrase]
- **R4:** An import of a twelve-minute fixture (three chunks) through `dettivo history import` with the tiny.en model produces one item with the expected merged transcript (golden), segments with words, progress events per chunk, and retained decoded audio; cancel mid-job leaves a cancelled item; the transfer and import fixtures still pass. Errors: a file over `max_import_seconds` is refused after the probe naming the limit. [paraphrase]
- **R5:** `dettivo-qa pipeline import-merge` runs the overlap fixtures through the engine CLI modes and the merger in CI and compares with goldens; the MCP harness gains a live `import_audio` step with the fixture WAV. Errors: as stated. [paraphrase]
- **R6:** `docs/history.md` gains the import section (formats, limits, chunking, what the progress events mean); the new config keys are documented and printed by `config print-default`; the delta register carries the progress fields; an ADR records the decoder and the merger's rules. Errors: as stated. [paraphrase]

## Boundaries

- No live windowed path, dual-source interleave or meeting events (S-24). [paraphrase]
- No diarization (S-25). [paraphrase]
- No GUI import dialog (S-20). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Long-audio import is the first user-visible use of the offline pipeline and the shared merger the meeting lane depends on; landing it against goldens now means the meeting spec inherits a tested core. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** an hour of audio in, a transcript out, proven against goldens through the engine CLI modes. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** every macOS content type imports, and `import_audio` gets its live harness step. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
