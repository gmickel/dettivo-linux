# Parakeet engine: parakeet.cpp binding, word timestamps and the meeting-capability decision

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-13 row: "`parakeet-engine-parakeet-cpp-sys-timestamps` | phase 2 | depends on S-05 | FR-E3, T5 decision; in-repo `parakeet-cpp-sys`; Sherpa-ONNX fallback path documented | Spike report on timestamp precision against a golden alignment; WER fixtures CPU and Vulkan; capability matrix update"
> masterplan FR-E3: "The Parakeet engine runs parakeet.cpp with TDT 0.6B v2 and v3 GGUF models, returns word timestamps and confidence, and is meeting-capable when the S-13 spike confirms timestamp precision against the merger's needs. The capability matrix reports the outcome honestly."
> masterplan T5: "One ggml family: whisper.cpp, parakeet.cpp, llama.cpp, Vulkan by default ... parakeet.cpp reports WER-0 parity with NeMo, publishes GGUF quantizations, exposes a C API and returns word timestamps, which solves the meeting gate both other ports gave up on."
> masterplan FR-E9: "GPU backend selection is per engine at start: Vulkan when a device enumerates and the model fits, else CPU; the choice and the reason are reported in `speech.providers.list` and `dettivo doctor`."
> masterplan 8.2: "`crates/dettivo-engine-parakeet` | Rust + parakeet.cpp | Engine binary over an in-repo `parakeet-cpp-sys` binding to `parakeet_capi.h`."
> masterplan risk table: "Parakeet timestamps too coarse for the merger | Parakeet stays dictation-only like macOS and Windows | S-13 spike decides early against a golden alignment; capability matrix stays honest either way"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Parakeet becomes the second speech engine and the first one that can say, with evidence, whether it may drive meetings. `dettivo-engine-parakeet` speaks the engine protocol from the whisper spec over an in-repo `parakeet-cpp-sys` binding to parakeet.cpp's C API, loads the TDT 0.6B v2 and v3 GGUF models from the catalogue, picks Vulkan or CPU at start and says why, returns segments with word timestamps and confidence, and has the CLI mode the benchmarks and fixtures use. A spike inside the spec measures the timestamps against a golden alignment and records the decision in the capability matrix: `parakeet` is meeting-capable, or dictation-only like the other ports, with the Sherpa-ONNX route documented as the fallback if it ever has to change. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/parakeet-cpp-sys`: a `-sys` crate vendoring parakeet.cpp as a git submodule (or a pinned source archive fetched at build time with a checksum, the way the whisper engine handles whisper.cpp) and building it through `cmake` with the ggml Vulkan backend when the toolchain has the Vulkan headers and shaderc, CPU otherwise; `bindgen` over `parakeet_capi.h`; the build follows ADR 0008's shape for the whisper engine so both engines share one story. [inferred]
- `crates/dettivo-engine-parakeet`: the engine binary implementing `load`, `unload`, `recognize` (inline 16 kHz PCM), `cancel`, `status` and progress events through `dettivo-engine-proto`, with a safe Rust wrapper over the C API (model handle, transcribe with timestamps, error strings), backend selection per FR-E9 (Vulkan when a device enumerates and the model fits, else CPU, the reason in `status`), a `--wav <file> [--model <path>] [--json]` CLI mode, and the same crash and timeout behaviour the supervisor expects. [paraphrase]
- Catalogue: `dettivo-speech/catalogue/v1.toml` gains provider `parakeet` with `parakeet-tdt-0.6b-v2` and `parakeet-tdt-0.6b-v3` (GGUF, sizes, SHA-256, licence, the quantisation used), so `dettivo speech download --provider parakeet` works through the existing verified download path; `[speech] parakeet_model_id` already exists. [paraphrase]
- Supervisor and selection: `speech.providers.list` reports `parakeet` with availability, backend and reason; the dictation session uses it when selected; the engine's capability flags (`supports_timestamps = true`, `supports_streaming = false`, languages per model: v2 English, v3 the 25 European languages) come from the engine's `status`. [paraphrase]
- The spike: `dettivo-qa pipeline parakeet-alignment` (an L2 scenario) runs the fixture set with a golden word alignment (a public forced-alignment fixture or one built once with a reference aligner and checked in) through both engines' CLI modes, computes per-word offset error (median and p95) and segment boundary error, and writes `alignment-report.json`; the decision rule is written down before the run: meeting-capable when p95 word offset is within the merger's tolerance (200 ms, the value the meeting merger spec will use for interleaving two sources). [inferred]
- Capability matrix: `system.capabilities.speech.providers[parakeet].meeting_capable` (Linux addition) and `docs/engines.md`'s matrix carry the outcome with a pointer to the report; an ADR records the decision and the Sherpa-ONNX fallback path (the diarization engine spec already brings Sherpa-ONNX in; a Parakeet-via-ONNX route would reuse it) in case the decision has to change. [paraphrase]
- WER: the whisper spec's WER fixture harness runs for Parakeet on CPU in CI (v2, the small fixture set) and on Vulkan on the development machine with thresholds per model. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Engine protocol as the whisper engine: `load { model_path, backend? }`, `recognize { pcm, language?, prompt? } -> { segments: [{ start_ms, end_ms, text, words: [{ start_ms, end_ms, text, confidence }] }], language, backend }`, `cancel`, `status`, progress events; fixtures under the engine proto crate. [paraphrase]
- `speech.providers.list` and `speech.models.status` list the Parakeet models and their readiness; `speech.selection.set { provider_id: "parakeet", model }` selects it; `system.capabilities.speech` carries `meeting_capable` per provider (delta register). [paraphrase]
- CLI: `dettivo-engine-parakeet --wav <file> --model <path> --json`, `dettivo speech download --provider parakeet --model parakeet-tdt-0.6b-v3`, `dettivo doctor` engine row for Parakeet with backend and reason. [paraphrase]
- Config: `[speech] parakeet_model_id` default `parakeet-tdt-0.6b-v3`; `[engines.parakeet] backend = "auto" | "vulkan" | "cpu"` matching the whisper section. [inferred]
- `just build-parakeet` and `build-parakeet-vulkan` recipes mirror the whisper ones; CI builds the CPU engine in the unit job and the Vulkan variant in the advisory job. [inferred]

## Edge Cases & Constraints

- No Vulkan device or the model does not fit: CPU with the reason; never a failed load. [paraphrase]
- A model file that fails the checksum is never loaded; the catalogue path reports it. [paraphrase]
- Audio longer than the model's window: the engine chunks with overlap inside `recognize` and stitches words by timestamp, the same as whisper's long-audio path, until the chunked pipeline spec generalises it. [inferred]
- Language not in the model (v2 asked for German): `recognize` answers `INVALID_PARAMS` naming the model's languages. [inferred]
- The alignment fixture cannot be produced: the spike reports `undecided` and the matrix says dictation-only; never a guess. [inferred]

## Acceptance Criteria

- **R1:** `parakeet-cpp-sys` builds parakeet.cpp from the pinned source with the CPU backend everywhere and the Vulkan backend where the toolchain has it, with a `bindgen` binding to `parakeet_capi.h`; `cargo build -p dettivo-engine-parakeet` succeeds in CI and here; a unit test loads a model and transcribes the short fixture through the safe wrapper. Errors: a missing model path is an error naming the path; a corrupt file is an error naming the reason from the C API. [paraphrase]
- **R2:** The engine binary passes the engine-protocol fixtures for `load`, `recognize` (segments with word timestamps and confidence), `cancel`, `status` and progress, under the supervisor's crash and timeout tests; the CLI mode prints the same JSON for a WAV. Errors: an unknown message is answered per the protocol's error shape. [paraphrase]
- **R3:** WER fixtures: v2 on CPU in CI and v2 and v3 on Vulkan here stay within the thresholds recorded in the fixture file; the backend chosen and its reason are in the report and in `dettivo doctor`. Errors: a threshold miss fails naming the fixture and the number. [paraphrase]
- **R4:** The catalogue lists both models with verified checksums; `dettivo speech download --provider parakeet` downloads and verifies; `speech.selection.set` picks it; a dictation through the mock microphone with Parakeet selected returns the fixture words (integration test with the v2 model cached locally, skipped by name when absent). Errors: as stated. [paraphrase]
- **R5:** The alignment spike runs both engines against the golden alignment, writes `alignment-report.json` with median and p95 word offsets and boundary errors, applies the written-down rule, and the outcome lands in `system.capabilities.speech.providers[].meeting_capable`, `docs/engines.md`'s matrix and an ADR with the Sherpa-ONNX fallback route. Errors: an unavailable fixture reports `undecided` and keeps dictation-only. [paraphrase]
- **R6:** `docs/engines.md` documents the Parakeet engine, its models, languages, backends and CLI mode; `[speech]` and `[engines.parakeet]` keys are documented and printed by `config print-default`; the `just` and Makefile recipes and the CI jobs build it. Errors: as stated. [paraphrase]

## Boundaries

- No meeting merger or diarization; those specs consume the decision. [paraphrase]
- No streaming recognition. [paraphrase]
- No benchmark calibration beyond the WER fixtures (S-16). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Parakeet is the fast, accurate dictation engine on GPU tiers, and its word timestamps are the only credible route to the meeting merger both other ports gave up on; deciding that early, with a report, keeps the meeting lane honest. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** a second ggml engine on the same Vulkan backend, with WER and alignment evidence. [strategy:Local engines and performance]
- **Complete speech workflows, proven by drives:** the spike's report is the evidence the meeting workflow builds on. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
