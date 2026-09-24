# Meeting transcription: the live windowed path, the two-source merger and live events

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-24 row: "`meeting-transcription-chunked-merge-live-events` | phase 4 | depends on S-23, S-15 | FR-G2, G3, G10 | Two-source fixtures with expected interleave; live event snapshot"
> masterplan FR-G2: "Transcription has two paths that share the merger. The live path windows audio with the macOS tuning as the starting point (target window 3000 ms, overlap 450 ms, tick 900 ms, boundary merge gap 1200 ms, cross-source suppression padding 800 ms, speech gating floors) and streams provisional then final segments. The offline path for finalization and import chunks at 300 s with 2 s overlap and a 5 s safety margin ... Both require a timestamp-capable engine, merge overlap by timestamp and interleave sources on absolute meeting time."
> masterplan FR-G3: "Known filler hallucinations are filtered before labels and analytics, and near-silent audio is guarded so Whisper does not invent text."
> masterplan FR-G10: "Starting a meeting with a non-timestamp engine selected is refused with a clear reason and a link to model settings."
> masterplan FR-A2: "Finalization transcribes every preserved take plus the system track, never a partial live view."
> masterplan FR-S5: "Linux adds topics `audio.level`, `model.download`, `engine.state` and `meeting.segment` for the OSD, GUI and bar widget, recorded as deltas."
> masterplan 8.10 Meeting, live: the approved `meeting-live.png` baseline (three panes, segments arriving in the live view, speaker chips).

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A meeting has words while it runs and a whole transcript when it stops. The live path windows both captured streams with the macOS tuning (3000 ms target window, 450 ms overlap, 900 ms tick, 1200 ms boundary merge gap, 800 ms cross-source suppression, the speech gating floors), transcribes each window through the selected timestamp-capable engine, streams provisional then final segments as `meeting.segment` events interleaved on absolute meeting time with a `source` of `you` or `remote`, and keeps the retained segment tail in the live checkpoint. Finalisation on stop reuses the offline chunked pipeline the import spec landed to transcribe every preserved take plus the system track, merges the two sources on absolute time, filters the known filler hallucinations and guards near-silence, and stores the segments on the meeting row so `meetings.get`, `transcripts.get` and the exports carry them. Recovery of a partial meeting finalises its takes the same way. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/dettivo-transcribe` grows the live path: `live::Windower` (per source: a ring of 16 kHz samples, windows of 3000 ms cut every 900 ms tick with 450 ms overlap, the speech gating floors from macOS deciding whether a window is sent), `live::Merger` (per source, provisional segments from the latest window replace the previous provisional tail; a segment older than the boundary merge gap of 1200 ms becomes final; finals merge across windows by timestamp the way the offline merger does) and `interleave` (two sources onto one absolute timeline with the 800 ms cross-source suppression: when both sources have speech in the same span, the microphone (`you`) keeps its words and the remote span within the padding is suppressed as echo, per the macOS rule). The offline `merger` and `filters` are shared; a `Source` enum (`you`, `remote`) rides on every segment. [paraphrase]
- Session (`crates/dettivo-meeting`): the worker feeds both capture streams into the windowers, drives one engine `recognize` per window through the supervisor (serialised, the remote source first when both are due; a window is skipped when the engine lags by more than two ticks, recorded in the journal), publishes `meeting.segment` events (`{ meeting_id, source, segment_id, provisional, start_ms, end_ms, text, words }`) through the event bus, writes the retained segment tail into the checkpoint every interval, and on stop enters `finalizing`: the takes and the system track go through the offline pipeline (chunker, engine, merger, filters), the two results interleave, the journal's gap markers become segment gap markers, and the segments land on the row; `meetings.stop`'s job reports `transcribing` progress per chunk as the contract's fixture expects. Recovery (`meetings.recover`) runs the same finalisation on the preserved takes. [paraphrase]
- Store: `meetings` rows gain `segments` (JSON with source, timestamps, words, gap markers) and `transcript` text; `meetings.get`, `transcripts.get { kind: meeting }` and `transcripts.search` (FTS over meeting segments joins the index) answer them; `meetings.search` searches titles and transcripts. [inferred]
- Gates: FR-G10 is already enforced at start; the live path also refuses to start when the selected engine loses timestamp support mid-session (settings change applies to the next session anyway). [paraphrase]
- Events and the OSD: `meeting.segment` is a registered topic; `meeting.state` carries the live segment count and the duration; the pill's Listening state during a meeting shows the meeting elapsed time when the meeting flag is set (the bar timer of the OSD baseline). [inferred]
- QA: two-source fixtures (the jfk clip as the remote track and a second short clip as the microphone, offset so they overlap in one span) with an expected interleave golden; the rig scenario `meeting_live` plays both through the null sinks, subscribes to `meeting.segment`, asserts provisional then final segments per source in order, stops and compares the finalised transcript with the golden; CI runs it with the mock fixtures (`DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_SYSTEM_AUDIO`) and `tiny.en`; a live event snapshot fixture under `crates/dettivo-proto/fixtures/events/` shows the payload shape. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `meeting.segment` event payload (Linux addition, registered with a fixture): `meeting_id`, `source`, `segment_id`, `provisional`, `start_ms`, `end_ms`, `text`, `words[]`, `gap_before_ms?`. [inferred]
- `meetings.stop` answers the contract's `transcribing` job message and `job.progress` reports finalisation chunks; `meetings.get` and `transcripts.get` carry `segments` and `transcript`; `meetings.search` and `transcripts.search` cover meeting text. [paraphrase]
- Config: `[meetings] live_window_ms = 3000`, `live_overlap_ms = 450`, `live_tick_ms = 900`, `boundary_merge_gap_ms = 1200`, `cross_source_padding_ms = 800`, `speech_floor_rms` (the gating floors), `live = true` (off runs finalisation only). [inferred]
- CLI: `dettivo meetings segments <id> [--follow]` (the live stream on the terminal), `dettivo events --follow --topic meeting.segment`. [inferred]

## Edge Cases & Constraints

- Engine slower than realtime: windows are skipped with a journal note and the live view shows a gap; finalisation still covers everything. [paraphrase]
- Both sources speaking at once: the microphone wins within the padding; the remote words outside it survive. [paraphrase]
- A window of silence: gated, no engine call, no invented text. [paraphrase]
- A device switch mid-meeting: the windower restarts on the new take with its offset; the gap marker appears in the live segments. [paraphrase]
- Cancel: live segments are discarded with the takes per policy. [inferred]
- The checkpoint's segment tail on recovery: shown as provisional until finalisation replaces it. [paraphrase]

## Acceptance Criteria

- **R1:** The live windower and merger are unit-tested with the macOS values: window cuts and overlaps per tick, gating of silent windows, provisional replacement and finalisation after the boundary gap, cross-window merge by timestamp; the interleave with the cross-source suppression against the two-source golden. Errors: a window from a non-timestamp result fails naming the engine. [paraphrase]
- **R2:** `meeting_live` on this machine and with the mock fixtures in CI: `meeting.segment` events arrive provisional then final per source in time order while the meeting runs, and the finalised transcript after stop matches the golden (interleave, gap markers, filler filter, silence guard). Errors: a lagging engine skips windows with a journal note and finalisation still matches. [paraphrase]
- **R3:** Finalisation transcribes every preserved take plus the system track (a device switch mid-meeting in the rig yields a transcript covering both takes with the gap marker), stores segments and transcript on the row, and `meetings.stop` reports `transcribing` progress per chunk; recovery of a partial meeting finalises the same way. Errors: as stated. [paraphrase]
- **R4:** `meetings.get`, `transcripts.get { kind: meeting }`, `meetings.search` and `transcripts.search` carry and find meeting text (fixtures pass, FTS covers segments); the exports of the history spec include meeting segments with source and timestamps in `txt`, `json` and `md`. Errors: as stated. [paraphrase]
- **R5:** The `meeting.segment` topic and payload are registered with a snapshot fixture and pass the events tests; `meeting.state` carries the segment count and duration; the pill shows the meeting elapsed time while a meeting runs (Quick Test). Errors: as stated. [paraphrase]
- **R6:** `docs/meetings.md` gains the transcription section (live tuning, interleave rule, finalisation, recovery); the config keys are documented and printed by `config print-default`; the CLI verbs are documented; an ADR records the live path and the suppression rule. Errors: as stated. [paraphrase]

## Boundaries

- No diarization or speaker labels beyond `you` and `remote` (S-25). [paraphrase]
- No notes, analysis, `srt`/`vtt` export formats or the GUI (S-26, S-27). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Words while the meeting runs and a complete transcript after it are the two things the macOS product promises; sharing one merger between live, finalisation and import keeps them consistent. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the live rig scenario and the interleave golden prove the meeting transcript end to end. [strategy:Complete speech workflows, proven by drives]
- **Local engines and performance:** the live path runs on the local engines at realtime with skips journaled, never invented text. [strategy:Local engines and performance]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
