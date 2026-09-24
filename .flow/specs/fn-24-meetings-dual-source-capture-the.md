# Meetings: dual-source capture, the session journal and recovery

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-23 row: "`meeting-dual-capture-session-journal-recovery` | phase 4 | depends on S-04, S-07 | FR-G1, G7, FR-A2 meeting behaviour | Virtual rig two-source capture; kill-and-recover test"
> masterplan FR-G1: "`meetings.start` captures microphone and the monitor of the default sink as two aligned streams, records a journal entry, and emits `recording` status; `stop`, `cancel`, `status`, `list`, `get`, `search` and `delete` follow the contract."
> masterplan FR-G7: "An interrupted meeting (crash, logout, power loss) is recoverable from a live checkpoint (`live-checkpoint.json` with the retained segment tail, schema version 1) and retained audio on next daemon start. Stale `recording` sessions are promoted to partial meetings with `is_partial`, `chunks_completed` and `chunks_total`, and the user is offered "recover" or "discard". Stop is idempotent with a visible "Stopping" state."
> masterplan FR-A2: "during a meeting the daemon restarts microphone capture on the new default device, preserves each take as a separate file with its start offset in metadata, and records a segment gap marker. Finalization transcribes every preserved take plus the system track, never a partial live view."
> masterplan FR-G9: "Before the first meeting the user acknowledges the disclosure notice, stored as `meeting.disclosure.acknowledged` with a timestamp ... `meetings.start` and `transcripts.import` for meetings accept `acknowledge_meeting_disclosure`"
> masterplan FR-G10: "Starting a meeting with a non-timestamp engine selected is refused with a clear reason and a link to model settings."
> masterplan 8.3: "Artifacts `$XDG_DATA_HOME/dettivo/dictations/<id>/`, `meetings/<id>/` with `microphone.wav` (and `microphone-2.wav` after device switches), `system.wav`, `transcript.json`, `notes.md`, `analysis.json`, `live-checkpoint.json`, `metadata.json`"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A meeting records both sides and survives anything. `meetings.start` opens two aligned PipeWire streams, the microphone and the monitor of the default sink, writes them as `microphone.wav` and `system.wav` under the meeting's directory with a journal entry and `metadata.json`, keeps a live checkpoint, and answers the contract's `meetings.stop|cancel|status|list|get|search|delete` with the meeting rows in the history store's one timeline. A microphone that vanishes mid-meeting restarts capture on the new default as `microphone-2.wav` with its offset and a gap marker; a daemon that dies mid-meeting promotes the stale session to a partial meeting on next start and offers recover or discard; stop is idempotent with a visible stopping state; the disclosure gate and the timestamp-engine gate are enforced. Transcription of the two tracks arrives with the next spec; this one makes the capture, the journal and the recovery solid on the virtual rig. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Capture (`crates/dettivo-audio`): a second capture kind, the monitor of the default sink (`stream.capture.sink = true` on the default sink's monitor, following the default like the microphone does), alongside the microphone; both streams at 16 kHz mono with one shared clock (PipeWire's stream time), each writing a WAV through the existing take writer with start offsets in nanoseconds relative to the meeting's `started_at`; the level meter for both feeds `audio.level` with a `source` field (`microphone`, `system`). [paraphrase]
- Session (`crates/dettivo-meeting`, new; depends on `dettivo-audio`, `dettivo-storage`, `dettivo-proto`): the meeting state machine (`recording`, `stopping`, `finalizing` reserved for the next spec, `stopped`, `cancelled`, `partial`), the journal (`journal.jsonl` under the meeting directory: start, device switch, gap, checkpoint, stop, with timestamps), the checkpoint writer (`live-checkpoint.json` schema version 1: meeting id, started_at, takes with offsets, the retained segment tail which is empty until transcription exists, `chunks_completed`, `chunks_total`), `metadata.json`, and the device-switch handler (on `audio.devices` default change the microphone stream restarts on the new device as `microphone-<n>.wav` with the offset and a journal gap marker; the system stream carries on). [paraphrase]
- Store: the history spec's schema gains the `meetings` table through a migration (macOS `MeetingSession` fields: id, title and title source, status, started_at, ended_at, duration_ms, language, stt provider and model, is_partial, chunks_completed, chunks_total, disclosure acknowledgement, audio paths) and the timeline lists meetings with dictations; `transcripts.list` with `kind = meeting` and `transcripts.get` for a meeting ref answer the meeting rows (their transcript empty until the next spec), which also retires the `MEETING_PENDING` replay list. [inferred]
- Recovery: at daemon start, any meeting row in `recording` with a checkpoint is promoted to `partial` (`is_partial`, `chunks_completed`, `chunks_total` from the checkpoint), the takes are preserved, and `meetings.status` lists it under `recoverable`; `meetings.recover { id }` (Linux addition) marks it for finalisation by the next spec (today it becomes `stopped` with its audio), `meetings.discard { id }` removes it per the artifact policy. [paraphrase]
- Gates: `meeting.disclosure.acknowledged` in the state store (the daemon's `state.toml`) with the timestamp; `meetings.start` without it answers `CONFLICT` with `kind = disclosureRequired` unless `acknowledge_meeting_disclosure = true` is passed, which records it; the macOS message text is the one shown; `meetings.start` with a non-timestamp engine selected answers `CONFLICT` with `kind = engineWithoutTimestamps` and the settings link. [paraphrase]
- Events: `meeting.state` per the contract (`recording`, `stopping`, `stopped`, `cancelled`, `partial`) with bounded live metadata (duration, takes, levels); `meeting.segment` is reserved for the next spec. [paraphrase]
- QA: the virtual audio rig (`scripts/qa/audio-rig.sh`) plays the microphone fixture into a null sink the daemon captures as its microphone and a second fixture into the default sink whose monitor is the system stream; `DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_SYSTEM_AUDIO` feed both tracks in CI; a kill-and-recover scenario starts a meeting, kills the daemon with SIGKILL, restarts it and asserts the partial meeting with its takes and the recover and discard outcomes; a device-switch scenario unloads the null sink mid-meeting and asserts `microphone-2.wav`, the offset and the gap marker. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `meetings.start { title?, acknowledge_meeting_disclosure?, language? }`, `meetings.stop { id? }` (idempotent, answers the stopping state then stopped), `meetings.cancel`, `meetings.status`, `meetings.list`, `meetings.get`, `meetings.search` (title and notes only until transcripts exist), `meetings.delete` per the contract fixtures under `fixtures/meetings/`; the meeting-kind `transcripts.*` fixtures start answering. [paraphrase]
- Linux additions with fixtures: `meetings.recover`, `meetings.discard`, `meetings.disclosure.get|acknowledge`, `audio.level.source`. [inferred]
- CLI: `dettivo meetings start|stop|cancel|status|list|get|delete|recover|discard`, `dettivo meetings disclosure [--acknowledge]`; `dettivo audio devices` shows the monitor source. [inferred]
- Config: `[meetings] keep_audio = true`, `checkpoint_interval_seconds = 15`, `artifacts`, `[audio] system_source = "default_monitor"`. [inferred]
- MCP: `start_meeting`, `list_meetings`, `get_meeting` stop answering guidance for the capture-level fields (transcript fields stay empty until the next spec). [inferred]

## Edge Cases & Constraints

- No default sink or its monitor unavailable: the meeting starts microphone-only and is treated as room audio (FR-G4 later), the journal says so. [paraphrase]
- Stop called twice: the second answers the current state, never an error. [paraphrase]
- Cancel: takes removed per the artifact policy, the row marked cancelled. [inferred]
- Disk full mid-meeting: the stream stops with an error state, the takes so far are kept, the journal records it. [inferred]
- A dictation session starting during a meeting: refused with `CONFLICT` `sessionActive` (the dictation fixture already expects it). [paraphrase]
- Checkpoint write failure: logged, capture continues, the next interval retries. [inferred]

## Acceptance Criteria

- **R1:** On the virtual rig (this machine) and with the mock fixtures (CI), `meetings.start` writes `microphone.wav` and `system.wav` with aligned offsets (the correlation check of the QA rig against both fixtures), the journal and `metadata.json`, and emits `recording` with levels per source; `stop` answers stopping then stopped idempotently. Errors: no monitor source starts microphone-only and says so. [paraphrase]
- **R2:** Device switch mid-meeting (unloading the null sink) restarts the microphone as `microphone-2.wav` with its offset and a gap marker in the journal while the system track continues; the metadata lists both takes. Errors: a switch that finds no device keeps the system track and records the gap. [paraphrase]
- **R3:** Kill-and-recover: SIGKILL on the daemon mid-meeting, restart, the meeting is `partial` with `is_partial`, `chunks_completed` and `chunks_total` from the checkpoint and its takes intact; `meetings.recover` keeps it, `meetings.discard` removes it per policy; `meetings.status` lists recoverables. Errors: a checkpoint that cannot be read still preserves the audio and reports the reason. [paraphrase]
- **R4:** Gates: `meetings.start` without the acknowledgement is `CONFLICT` `disclosureRequired`, `acknowledge_meeting_disclosure = true` records it with the timestamp and the macOS text is exposed through `meetings.disclosure.get`; a non-timestamp engine selected is `CONFLICT` `engineWithoutTimestamps` with the settings link; a dictation during a meeting is `CONFLICT` `sessionActive`. Errors: as stated. [paraphrase]
- **R5:** Every `meetings.*` contract fixture and the meeting-kind `transcripts.*` fixtures pass against the live daemon (stateful ones by name in the daemon tests), the replay's `MEETING_PENDING` list is gone, the MCP meeting tools answer the capture-level fields, and the migration adds the meetings table with the timeline listing both kinds. Errors: as stated. [paraphrase]
- **R6:** `docs/meetings.md` describes capture, the journal, recovery and the gates; the config keys are documented and printed by `config print-default`; the deltas are registered with fixtures; an ADR records the two-stream capture and the checkpoint schema. Errors: as stated. [paraphrase]

## Boundaries

- No transcription, merger or live segments (S-24); no diarization (S-25); no notes, analysis, export or the GUI (S-26, S-27). [paraphrase]
- No pausing of MPRIS or cues in meetings beyond the existing flag. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The Windows port's meeting lane failed on recovery and device loss; a capture core that journals, checkpoints and recovers on the virtual rig before any transcription exists is what makes the rest of the meeting specs safe. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** two-source capture, device loss and recovery are proven on the rig before transcription lands. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** the `meetings.*` methods and the disclosure gate match macOS and the Windows addition. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
