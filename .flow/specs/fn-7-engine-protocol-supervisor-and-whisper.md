# Engine protocol, supervisor and the Whisper engine process

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 1): "Rust direction recorded"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-05 row: "`engine-protocol-supervisor-whisper-engine` | phase 1 | depends on S-02 | FR-E1, E2, E6, E7, E8, E9, FR-D7, T4, T5, 8.8 | Engine protocol fixtures; supervisor crash and idle-unload tests; Whisper WER fixtures CPU and Vulkan via CLI mode"

## Goal & Context

<!-- Goal & Context: 30% [user], 40% [paraphrase], 30% [strategy] -->

Speech recognition runs in a supervised child process that the daemon starts on demand, keeps warm, and terminates after an idle timeout so GPU memory is returned, with one framed protocol shared by every engine and a CLI mode on every engine binary so benchmarks and fixtures exercise the exact inference path. This spec delivers the protocol crate, the supervisor with crash restart and idle unload, the `SttEngine` trait that mirrors the macOS capability contract, and the first engine: whisper.cpp with Vulkan by default and CPU fallback. [paraphrase]

Inference lives out of process because whisper.cpp and llama.cpp each vendor ggml and collide when linked together, a segfault in native inference must not kill the hotkey daemon, and unloading a process is the only reliable way to return GPU memory (decision T4). One ggml family with Vulkan covers NVIDIA, AMD and Intel with one binary (decision T5). [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- `dettivo-engine-proto`: the versioned protocol over a socketpair passed at spawn (stdio fallback for CLI-style runs). Frames are length-prefixed JSON messages with optional binary attachments (16 kHz mono PCM). Requests: `load { model, backend_preference }`, `unload`, `recognize { pcm, language, prompt, timestamps }`, `generate { prompt, options }`, `diarize { pcm }`, `cancel { request_id }`, `status`. Events: `progress`, `partial`, `loaded { backend, reason }`, `error`. Every message is fixture-tested against a checked-in suite the way the IPC contract is. [paraphrase]
- `dettivo-speech`: the `SttEngine` trait mirroring the macOS `STTEngine` capability contract (`supports_timestamps`, `supports_streaming`, `supported_languages`, `max_duration_seconds`, `supports_custom_vocabulary`, `supports_model_deletion`, `preload`, `recognize`, `cancel`), with a `ProcessEngine` implementation that proxies to a child through the protocol. A session freezes its engine and model at start. [paraphrase]
- The supervisor in the daemon: engines are discovered by name on `PATH` or in `[engines] directory`; spawned on first need with the selected model; `load` sent; kept warm; `unload` then terminate after the idle timeout (`[engines] stt_idle_seconds`, default 300; `llm_idle_seconds`, default 600); restarted on the next request after a crash with backoff; three crashes in a row mark the engine degraded, reported in `system.health` (`ok` stays true, the degradation is logged and shown by `dettivo doctor`) and cleared by a successful load. Preload sources `startup`, `model_selection`, `session_start` are coalesced, never download implicitly, and fail non-fatally. [paraphrase]
- `dettivo-engine-whisper`: whisper.cpp through `whisper-rs` with the Vulkan feature; on `load` tries Vulkan when a device enumerates and the model fits, else CPU, and reports the choice and reason in `loaded`; uses whisper.cpp's built-in VAD; returns timestamped segments; accepts a language and a vocabulary prompt; `DETTIVO_FORCE_CPU=1` forces CPU. CLI mode: `dettivo-engine-whisper --wav <file> --model <path> [--language xx] [--prompt text] --json` prints the same JSON the protocol would. [paraphrase]
- Models for tests: a small whisper ggml model (`tiny.en`) fetched by a script with a pinned checksum into the model directory layout from ADR 0005 (`models/whisper/tiny.en/`), cached in CI; the catalogue and downloads proper arrive with S-06. [inferred]
- Logs from engines are captured by the supervisor with the last lines kept for error reports, redacted of transcript text. [paraphrase]

## API Contracts

<!-- API Contracts: 60% [paraphrase], 40% [inferred] -->

- Protocol frame: `u32 length` + JSON `{ "v": 1, "id": ..., "type": "request"|"event", "name": ..., "payload": ..., "attachments": [ { "kind": "pcm16k", "bytes": n } ] }` followed by the attachment bytes; documented in `docs/engines.md` with the fixture suite under `crates/dettivo-engine-proto/fixtures/`. [inferred]
- `recognize` result: `{ text, language, segments: [{ start_ms, end_ms, text }], duration_ms, backend }`. [paraphrase]
- Config keys `[engines] directory = ""`, `stt_idle_seconds = 300`, `llm_idle_seconds = 600`, and `[speech] provider = "whisper"`, `model = "large-v3-turbo"` (the selection keys S-06 completes), documented in `docs/config.md`. [user]
- `dettivo doctor` reports each engine binary found, its backend choice from the last load and its degraded state. [inferred]

## Edge Cases & Constraints

- An engine binary missing from `PATH` and the configured directory is a clear error naming the binary and the directories searched. [inferred]
- A crash during `recognize` fails that request on the session with the engine's last redacted stderr lines, never the daemon. [paraphrase]
- Vulkan present but the model does not fit falls back to CPU with the reason reported. [paraphrase]
- `cancel` for an unknown request id is not an error. [inferred]

## Acceptance Criteria

- **R1:** Every protocol message and event round-trips byte-stable through `dettivo-engine-proto` from a checked-in fixture suite, including a `recognize` with a PCM attachment. Errors: a malformed frame is rejected with the field named. [paraphrase]
- **R2:** The supervisor spawns an engine on first use, keeps it warm across requests, unloads and terminates it after the configured idle timeout (proven with a short timeout), restarts it with backoff after a forced crash, and marks it degraded after three consecutive crashes, all visible in the daemon log and `dettivo doctor`. Errors: a missing binary names the binary and the directories searched. [paraphrase]
- **R3:** `dettivo-engine-whisper --wav fixture.wav --model tiny.en --json` on CPU returns a transcript whose word error rate against the fixture's reference is under the stated threshold, with timestamped segments; the same run over the protocol from the daemon returns the same JSON. Errors: a missing model names the path. [paraphrase]
- **R4:** On a machine with a Vulkan device the engine reports backend `vulkan` at load with the same WER, and `DETTIVO_FORCE_CPU=1` reports `cpu`; the backend and reason appear in the `loaded` event and `dettivo doctor`. Errors: no error surface beyond the reason string. [paraphrase]
- **R5:** The `SttEngine` trait exposes the macOS capability contract and `ProcessEngine` proxies it; a session-level `preload` from `startup`, `model_selection` and `session_start` is coalesced and never downloads. Errors: a preload failure is logged privacy-safe and does not fail the caller. [paraphrase]
- **R6:** The `[engines]` and `[speech]` keys are documented, printed by `config print-default`, and change the idle timeout and the engine directory without a restart. Errors: an invalid value fails validation naming the key. [user]
- **R7:** No log line from the supervisor or the engine contains transcript text, prompts or audio, proven with marker strings through the CLI mode and the protocol path. Errors: no error surface. [paraphrase]

## Boundaries

- No Parakeet, LLM or diarization engines; they arrive with S-13, S-33 and S-24 on this protocol. [paraphrase]
- No model catalogue, downloads or `speech.*` methods; S-06 builds those on the model layout this spec uses. [paraphrase]
- No dictation session; S-07 is the first caller of `SttEngine`. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The Windows port fought idle VRAM retention for weeks; process-per-engine makes releasing GPU memory a process exit, and the CLI mode gives benchmarks and fixtures the exact inference path for free. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the first real recognizer, proven by WER fixtures on both backends. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** the engine capability contract mirrors macOS so provider reporting stays portable. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
