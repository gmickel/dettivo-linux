# PipeWire microphone capture: devices, default following, levels and takes

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 1): "Rust direction recorded"
> user (turn 13): "ALL needs to be able to be configured via config files as is the linux way"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-04 row: "`pipewire-mic-capture-devices-levels` | phase 1 | depends on S-02 | FR-A1, A2, A3, A4, T8 | Rig fixture captured to WAV byte-equal within tolerance; hot-swap test"

## Goal & Context

<!-- Goal & Context: 30% [user], 40% [paraphrase], 30% [strategy] -->

The daemon can hear the user: it captures the microphone through PipeWire directly, follows the default source unless a device is pinned in `config.toml`, asks PipeWire for 16 kHz mono so no resampler runs in the daemon, writes takes as WAV files, publishes input levels for the meters, and treats a device swap as an event the session layer can act on. This is the capture path every dictation and meeting spec plugs into, and the virtual audio rig from the QA spec is how it is proven without a microphone. [paraphrase]

PipeWire is chosen over CPAL because monitor capture, default-device following and consistent clocks for dual-source meetings need stream properties CPAL does not expose, and asking PipeWire for the target rate removes a resampler from the daemon (decision T8). [paraphrase]

Everything about capture is configurable in the file, never only in a GUI. [user]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- `dettivo-audio` owns capture: a `Capture` that opens a PipeWire capture stream on the default source (or the pinned node name), requests `s16` mono at 16 kHz with PipeWire-side conversion, delivers PCM chunks and level samples to the caller on a channel, and can be started, stopped and re-targeted. Device enumeration lists source nodes with their names, descriptions and whether one is the default. [paraphrase]
- Default following: when the default source changes and no device is pinned, the capture emits a `DeviceChanged` event carrying the old and new node; the caller decides whether to end (dictation) or restart on the new device (meeting). A restart opens a new take. [paraphrase]
- Takes: a `TakeWriter` writes 16 kHz mono 16-bit WAV to `<artifact dir>/microphone.wav`, `microphone-2.wav` and so on, records each take's start offset in a `takes.json` sidecar, and a gap marker when a swap interrupted capture. Retention of dictation audio follows `[audio] keep_dictation_audio` (default false); meeting audio is always retained. [paraphrase]
- Levels: RMS and peak per 50 ms window, published on an internal broadcast the event layer will forward on the `audio.level` topic when the events spec lands; the payload type lives in `dettivo-proto` now. [inferred]
- Configuration: `[audio] input_device = ""` (empty follows the default), `keep_dictation_audio = false`, `level_interval_ms = 50`, documented in `docs/config.md`. QA mode with `DETTIVO_MOCK_MIC=<wav>` replaces the PipeWire stream with the fixture played at real time, through the same `Capture` API, so the pipeline above it does not know the difference. [paraphrase]
- `dettivo doctor` reports the default source, the pinned device when set, and whether PipeWire answered; `dettivo-audio` also ships a small `dettivo-audio-probe` test helper or a `--probe` mode on the daemon is not added; the doctor is the user-facing view. [inferred]

## API Contracts

<!-- API Contracts: 60% [paraphrase], 40% [inferred] -->

- `Capture::open(target: Target) -> Result<Capture>` where `Target` is `Default` or `Node(name)`; `Capture::events()` yields `Pcm(chunk)`, `Level { rms, peak }`, `DeviceChanged { from, to }`, `Ended { reason }`. [inferred]
- `devices::list() -> Vec<Device { name, description, is_default }>`, `devices::default_source() -> Option<Device>`. [inferred]
- `TakeWriter::new(dir) -> TakeWriter`, `start_take() -> TakeHandle`, `write(pcm)`, `finish()`; `takes.json` records `{ index, file, start_offset_ms, gap_before }`. [inferred]
- `audio.level` event payload: `{ rms: f32, peak: f32, source: "microphone" | "system" }`, added to `dettivo-proto` events and the deltas register. [paraphrase]
- Config keys `[audio]` above with defaults; `config.print_default` and `docs/config.md` carry them. [user]

## Edge Cases & Constraints

- No PipeWire (a CI container): capture reports a clear error naming PipeWire and the QA fixture path still works. [inferred]
- A pinned device that disappears: capture ends with a recoverable error naming the device; it does not silently follow the default. [inferred]
- Zero-signal input (a muted or dead microphone) must not hang a caller: levels still flow at the interval, at zero. [paraphrase]
- The fixture microphone plays at real time so timing-dependent code sees realistic pacing; a `DETTIVO_MOCK_MIC` file that is not 16 kHz mono is converted once on open. [inferred]

## Acceptance Criteria

- **R1:** With the virtual audio rig playing a fixture into a null sink whose monitor is the default source, `Capture::open(Default)` delivers 16 kHz mono PCM that, written by `TakeWriter`, matches the fixture within a stated tolerance (sample-level correlation above 0.99 after delay alignment). Errors: no PipeWire returns an error naming PipeWire. [paraphrase]
- **R2:** With no device pinned, unloading the default sink mid-capture yields `DeviceChanged` (or `Ended` with a recoverable reason when no source remains) within one second, and re-opening on the new default starts a second take with its offset and a gap marker in `takes.json`. Errors: a pinned device vanishing ends capture with the device named. [paraphrase]
- **R3:** Level samples arrive at the configured interval with RMS and peak in 0..1, including zeros during silence; the `audio.level` payload type round-trips through `dettivo-proto` and appears in the deltas register. Errors: no error surface. [paraphrase]
- **R4:** `[audio] input_device`, `keep_dictation_audio` and `level_interval_ms` are read from `config.toml`, documented in `docs/config.md`, printed by `config print-default`, and a dictation take is deleted at session end unless `keep_dictation_audio` is true, while a meeting take is kept. Errors: an unknown device name is a validation error naming the key. [user]
- **R5:** `DETTIVO_MOCK_MIC=<wav>` drives the same `Capture` API from a file at real time, and the R1 test runs on that path in CI where PipeWire is absent, reporting the rig path as skipped rather than passed. Errors: a missing file is refused at start by name. [paraphrase]
- **R6:** `dettivo doctor` reports the default source, the pinned device and PipeWire reachability. Errors: no error surface beyond the report. [inferred]

## Boundaries

- No system-audio (sink monitor) capture, no dual-source clock alignment; that is the meetings capture spec. [paraphrase]
- No dictation session, no state machine, no `dictation.*` methods; S-07 consumes this crate. [paraphrase]
- No event subscriptions on the socket; the payload type is defined here, the topic is served when S-07 lands the events layer. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- Capture is the first thing every later slice depends on, and the Windows port never proved real microphone input on its development host; this spec proves the real PipeWire path on Thor and the fixture path in CI from day one. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the capture path with a rig-backed proof is the base of every workflow. [strategy:Complete speech workflows, proven by drives]
- **Omarchy-native, beautiful by default:** PipeWire is the native audio system on Omarchy and every modern desktop. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
