# Dictation session: state machine, IPC methods and the event stream

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-07 row: "`dictation-session-state-machine-ipc-events` | phase 1 | depends on S-04, S-05 | FR-H1, FR-E7, FR-S5, FR-M1 | `dictation.*` fixtures; event stream snapshot; state machine unit tests including concurrent starts"
> masterplan FR-H1: "The CLI exposes `dictation start`, `stop`, `toggle`, `cancel`, `status` and `reinsert-last` so any compositor binding can drive push-to-talk, toggle, cancel and re-insert."
> masterplan FR-S5: "`events.subscribe` and `events.unsubscribe` follow the contract with server notification `events.notify` carrying `subscription_id`, `topic`, `timestamp` and `payload`; topics `dictation.state`, `meeting.state`, `job.progress`, overflow topic `events.overflow` with a drop count when the per-subscriber buffer (default 256) fills."
> masterplan Appendix D: "Reserve the session-active flag synchronously before the first await; a reentrancy window otherwise allows a second session."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A compositor binding can now dictate: `dictation.start` reserves the single session, captures the microphone through the audio layer, `dictation.stop` transcribes the take through the frozen engine and model, applies the raw text pipeline, keeps the result as the last transcript and hands it to the insertion layer; `cancel`, `status` and `toggle` behave per contract; every transition is published on the event stream so the OSD, the bar and the GUI show the same state. This spec delivers the session state machine, the four `dictation.*` methods, `events.subscribe`, `events.unsubscribe` and `events.notify` with the overflow rule, the `dictation.state`, `audio.level` and `engine.state` publishers, and the `dettivo dictation` and `dettivo events --follow` verbs. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- `dettivo-session` (new crate, depends on `dettivo-proto`, `dettivo-audio`, `dettivo-speech`): the dictation state machine `idle → recording → transcribing → inserting → idle` with `cancelled` and `failed` exits, one session per daemon, the active flag reserved synchronously before the first await, a `JobStatus` per session with id, state, progress and timestamps, and the frozen `SessionPolicy` (engine, model, language, mode, config hash) captured at start. [paraphrase]
- Recording: the session opens a capture through `AudioService` (pinned device or default), meters levels onto `audio.level`, writes the take through `TakeWriter` into the session directory under `$XDG_STATE_HOME/dettivo/sessions/<id>/`, and ends the session with a recoverable `failed` state naming the device when the capture reports `DeviceLost` or `DeviceChanged` during dictation (FR-A2, dictation half). A near-silent take (peak under a threshold) is transcribed but flagged `silent` so the text pipeline drops hallucinated filler. [paraphrase]
- Transcribing: `SttEngine::recognize` with the frozen policy, the vocabulary prompt from `[dictation] vocabulary`, a bounded duration (`max_duration_seconds` from the capability contract, default 300 for dictation), and cancellation through `cancel`. The raw pipeline applies dictionary corrections, the replacement table and spoken punctuation (FR-M1) from `[dictation]` keys; `deterministic_polish` and `enhanced` are accepted mode identifiers but answer `NOT_IMPLEMENTED` until S-14. [paraphrase]
- Result: the last transcript is kept in memory (and in the session directory as `transcript.json`) with a `TranscriptRef { kind: dictation, id }`; `dictation.stop` returns it. Insertion goes through an `Inserter` trait the session calls with the text and the target captured at start; this spec ships a `ClipboardOnly` implementation over `wl-clipboard-rs` that reports `copied_to_clipboard`, and S-08 replaces it with the backend chain. `reinsert-last` calls the same trait with the last transcript. [inferred]
- Events: an `EventBus` in the daemon with per-subscriber bounded buffers (default 256), `events.subscribe { topics }` returning a `subscription_id`, `events.notify` server notifications on the subscriber's connection, `events.overflow` with the drop count when a buffer fills, unsubscription on `events.unsubscribe` and on disconnect. Publishers: the session (`dictation.state`), the capture (`audio.level` at `[audio] level_interval_ms`), the supervisor (`engine.state` on spawn, loaded, idle unload, crash, degraded). [paraphrase]
- Audio retention: the take is deleted at session end unless `[audio] keep_dictation_audio` is on (FR-A4). [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dictation.start { language, mode }` → `{ job }`; `dictation.stop` → `{ ref, job }`; `dictation.cancel` → `{ job }`; `dictation.status` → `{ is_active, job, context_pack }` in the contract shapes with the existing fixtures; a second `start` while active is `CONFLICT`; `stop` or `cancel` with no session is `NOT_FOUND`. [paraphrase]
- `events.subscribe { topics }`, `events.unsubscribe { subscription_id }`, notification `events.notify { subscription_id, topic, timestamp, payload }`; `dictation.state` payload `{ job_id, state, previous_state, reason }`; `engine.state` payload `{ binary, state, model, backend, reason }`; `audio.level` payload as already typed. Unknown topics are `INVALID_PARAMS`. [paraphrase]
- CLI: `dettivo dictation start --language --mode | stop | cancel | status | toggle | reinsert-last`, `dettivo events --follow [--topic ...]` printing one JSON line per notification; `toggle` and `reinsert-last` are Linux additions recorded as deltas. [paraphrase]
- Config: `[dictation] language = "auto"`, `mode = "raw"`, `vocabulary = []`, `replacements = []`, `spoken_punctuation = true`, `max_duration_seconds = 300`, `silence_peak_threshold = 0.01`, documented in `docs/config.md`. [inferred]
- `system.health.recording_state` reflects the session (`idle`, `recording`, `transcribing`), `active_jobs` counts it. [paraphrase]

## Edge Cases & Constraints

- Two concurrent `dictation.start` calls: exactly one wins, the other is `CONFLICT`, proven with a test that races them on two connections. [paraphrase]
- Hotkey release before the engine is warm: the session records anyway; transcription waits for the load with progress on `dictation.state` (`transcribing`, reason `loading engine`). [inferred]
- The engine crashes during `recognize`: the session ends `failed` with the redacted reason, the take is kept for the session directory when audio retention is on, and the next start restarts the engine through the supervisor. [paraphrase]
- A subscriber that never reads: its buffer overflows, it receives `events.overflow` with the count, and other subscribers are unaffected. [paraphrase]
- Cancel during transcription: the engine request is cancelled, the state is `cancelled`, no transcript is kept. [paraphrase]
- Transcript text never appears in logs or event payloads; events carry ids and states. [paraphrase]

## Acceptance Criteria

- **R1:** The state machine unit tests cover every transition including concurrent starts (one wins, one `CONFLICT`), cancel in every state, device loss during recording ending the session recoverably, and the engine crash path; the active flag is reserved before the first await. Errors: `stop` or `cancel` without a session is `NOT_FOUND`. [paraphrase]
- **R2:** With the mock microphone fixture and the tiny.en engine, `dictation.start` then `dictation.stop` returns a transcript ref whose text contains the fixture's key words, the take is written and then removed unless audio retention is on, and `dictation.status` reports `is_active` and the job through the whole session. Errors: a missing selected model fails the session with `NOT_FOUND` naming the model and the recommended action. [paraphrase]
- **R3:** The `dictation.*` and `events.*` fixtures pass against the live daemon, and an event stream snapshot test records the exact `events.notify` sequence for one dictation (`recording`, `transcribing`, `inserting`, `idle`) plus at least one `audio.level` and one `engine.state` notification. Errors: subscribing to an unknown topic is `INVALID_PARAMS` naming it. [paraphrase]
- **R4:** A subscriber that stops reading receives `events.overflow` with a drop count once its 256-slot buffer fills while another subscriber receives every event. Errors: no error surface beyond the overflow notification. [paraphrase]
- **R5:** The session freezes engine, model, language and mode at start: a `[speech]` change during a session applies to the next one, proven by a test that edits the config mid-session. Errors: no error surface. [paraphrase]
- **R6:** `dettivo dictation start|stop|cancel|status|toggle|reinsert-last` and `dettivo events --follow` work end to end in QA mode with the mock microphone, `toggle` starting when idle and stopping when recording; the raw pipeline applies the dictionary, replacements and spoken punctuation from `[dictation]` on a golden set. Errors: `--mode enhanced` answers `NOT_IMPLEMENTED` until S-14. [paraphrase]

## Boundaries

- No insertion backends beyond the clipboard-only inserter; S-08 delivers the chain and the target guards. [paraphrase]
- No history store; the last transcript lives in the session directory until S-11 persists it. [paraphrase]
- No polish or enhanced modes; S-14 implements them behind the accepted identifiers. [paraphrase]
- No OSD; S-10 subscribes to these events. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The first dogfood (S-12) is "hold F9 in ghostty and text lands"; the session and the event stream are the spine every other slice hangs on. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the first complete dictation, proven with the mock microphone and the real engine. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** `dictation.*` and `events.*` answer with the contract shapes and fixtures. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
