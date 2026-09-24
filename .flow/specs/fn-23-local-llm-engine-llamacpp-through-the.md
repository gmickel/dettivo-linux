# Local LLM engine: llama.cpp through the engine protocol, the GGUF catalogue and the local provider

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-33 row: "`llm-engine-llama-cpp-local-provider` | phase 2 | depends on S-05, S-14 | FR-M6 local provider, FR-M8 catalogue, T7; onboarding Enhanced step wiring | Engine protocol fixtures for `generate`; rewrite goldens with a small model; idle unload and VRAM release test on Thor"
> masterplan FR-M6: "The LLM provider layer has three providers behind one interface: `local` (the llama.cpp engine process with a catalogue GGUF, default once downloaded) ..."
> masterplan FR-M8: "The Enhanced and meeting-analysis models are the macOS catalogue in GGUF form: Qwen3 4B Instruct 2507 as the default for both, Qwen3 1.7B as the fast option, Qwen3 4B as the quality option, Qwen3 8B for meeting analysis, quantized to match the macOS 4-bit footprint."
> masterplan T7: "Local LLM engine in v1 with Ollama and endpoints as alternatives ... The engine protocol already exists for STT, `llama-cpp-2` is stable and tracks upstream, and the same Vulkan backend applies. The user still chooses in onboarding; the download is optional."
> masterplan 8.8: "Lifecycle: the daemon spawns an engine on first need with the selected model, sends `load`, keeps it warm, and sends `unload` then terminates it after the idle timeout (default 5 minutes for STT, 10 for the LLM; both configurable) ... CLI mode: every engine binary accepts `--wav <file> --model <path> --json` (or `--prompt` for the LLM) and prints the same JSON the protocol would"
> masterplan FR-M7: "Thinking blocks are stripped from model output, empty or degenerate rewrites fall back to the Polish result, and the output guard rejects rewrites that drop code, paths or numbers present in the input."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Enhanced works with no Ollama and no endpoint. `dettivo-engine-llm` runs llama.cpp through the engine protocol the speech engines already use: `load` a catalogue GGUF on Vulkan or CPU and say which, `generate` with the Polish prompt profile streaming partials, `cancel`, `status`, an idle unload after ten minutes that returns the VRAM, and the `--prompt` CLI mode for goldens and benchmarks. The LLM catalogue lists the macOS models in GGUF form with verified checksums (Qwen3 4B Instruct 2507 as the default, 1.7B fast, 4B quality, 8B for meeting analysis), downloads through the existing verified path, and the `local` provider of the Polish spec stops being a stub: it becomes the default once a model is downloaded, with the same guards and fallback. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/dettivo-engine-llm`: the engine binary on the shared `dettivo-engine-proto` host and backend layer, over the `llama-cpp-2` crate (with its `llama-cpp-sys-2` build; the Vulkan feature where the toolchain has it, CPU otherwise, following the whisper and Parakeet build stories and the advisory CI job pattern); messages `load { model_path, backend?, context_length? }`, `generate { prompt_profile, system, user, max_tokens, temperature, stop } -> partial events then { text, tokens, finish_reason, backend }`, `cancel`, `unload`, `status`; thinking blocks are left to the language crate's guard (the engine returns raw text); `--prompt <text> --model <path> --json` CLI mode; the supervisor's crash, timeout and degrade behaviour with the LLM idle timeout (`[engines.llm] idle_unload_seconds = 600`). [paraphrase]
- Catalogue: `dettivo-speech/catalogue/v1.toml` gains an `llm` provider section (or a sibling `llm.toml` read by the same code) with `qwen3-4b-instruct-2507` (default for polish and analysis), `qwen3-1.7b` (fast), `qwen3-4b` (quality), `qwen3-8b` (meeting analysis), each a GGUF at the 4-bit footprint with size, SHA-256, licence and role hints; `llm.models.status|download|cancel|delete` mirror the speech methods (Linux additions with fixtures), `dettivo llm download|delete --model`, `[llm] model = "qwen3-4b-instruct-2507"`, `[llm] analysis_model`. [paraphrase]
- Provider: the `local` provider in `dettivo-language` calls the daemon's LLM engine through the supervisor (a `generate` request with the prompt profile), reports `Availability::Available` when the selected model is on disk and the engine binary exists, `unavailable` naming the download otherwise; `[llm] provider = "auto"` picks `local` when available, then `ollama`, then an endpoint; `dettivo llm test "<text>"` and `polish.test` run through it. [paraphrase]
- Onboarding wiring: `llm.models.*` is what the first-run Models step's Enhanced tick uses (the first-run spec consumes it). [paraphrase]
- Goldens: the Polish spec's Enhanced fixture cases run through the real engine with the smallest catalogue model (Qwen3 1.7B, cached locally; skipped by name when absent, and in CI unless the model cache has it) and must pass the guards; the alignment of outputs with the macOS goldens is recorded as a report, not gated, since sampling differs. [inferred]
- Idle unload and VRAM: on this machine, a test loads the model on Vulkan, generates, waits past a short configured idle timeout, and asserts the engine process is gone and the daemon's `speech.engines` (or the new `llm.engine` status) reports unloaded; VRAM release is observed through the engine's reported backend memory before and after (via the engine's `status`), recorded in the evidence. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Engine protocol: `generate` request and `partial` event shapes added to `dettivo-engine-proto` with fixtures; `status` gains `memory_bytes` when the backend reports it. [inferred]
- Daemon Linux additions with fixtures: `llm.models.status|download|cancel|delete`, `llm.engine.status`; `llm.providers.list` reports `local` with availability and the selected model; `system.capabilities.llm.local_available`. [inferred]
- CLI: `dettivo-engine-llm --prompt <text> --model <path> [--system] --json`, `dettivo llm download|delete|status`, `dettivo llm test` through the local provider. [paraphrase]
- Config: `[llm] model`, `analysis_model`, `[engines.llm] backend`, `idle_unload_seconds`, `context_length`, `max_tokens`. [inferred]
- Build: `just build-llm`, `build-llm-vulkan`, CI unit job builds CPU, an advisory `llm-vulkan` job like the others. [inferred]

## Edge Cases & Constraints

- Model too large for the GPU: the engine falls back to CPU with the reason, and the provider's timeout still bounds the rewrite. [paraphrase]
- Generation slower than `[llm] timeout_ms`: the Polish result is inserted with `provider_unavailable` and the generation is cancelled. [paraphrase]
- Engine crash mid-generation: the provider falls back; the supervisor restarts on the next request; three crashes degrade it in health. [paraphrase]
- Two rewrites at once (dictation and a `polish.test`): the engine serialises requests; the second waits within its timeout. [inferred]
- No model downloaded: `local` reports unavailable naming `dettivo llm download`, `auto` moves on to Ollama. [paraphrase]

## Acceptance Criteria

- **R1:** `dettivo-engine-llm` builds on CPU everywhere and on Vulkan where the toolchain has it, passes the engine-protocol fixtures for `load`, `generate` with partials, `cancel`, `status` and `unload` under the supervisor's crash and timeout tests, and the CLI mode prints the same JSON. Errors: a missing or corrupt model is an error naming the path and the reason. [paraphrase]
- **R2:** The catalogue lists the four models with verified checksums; `dettivo llm download` downloads and verifies; the `llm.models.*` fixtures pass; `[llm] model` selects. Errors: an unknown model id is `NOT_FOUND` naming the catalogue. [paraphrase]
- **R3:** The `local` provider rewrites through the engine: `dettivo llm test` and `polish.test` in `enhanced` mode return a guarded rewrite with the 1.7B model cached locally, a dictation in `enhanced` mode inserts it, and the Polish spec's guard and fallback tests pass unchanged with the real provider behind them (skipped by name without the model). Errors: as stated. [paraphrase]
- **R4:** Idle unload on this machine: after the configured idle the engine process is gone, the status reports unloaded, and the reported backend memory drops; the evidence records the numbers. Errors: an engine that ignores `unload` is terminated by the supervisor and logged. [paraphrase]
- **R5:** `[llm] provider = "auto"` prefers `local` when the model is on disk, then Ollama, then an endpoint (unit tests over the availability matrix); `system.capabilities.llm.local_available` and `dettivo doctor` report it. Errors: as stated. [paraphrase]
- **R6:** `docs/engines.md` and `docs/polish.md` document the engine, the catalogue, the provider order and the idle unload; the config keys are documented and printed by `config print-default`; the deltas are registered with fixtures; an ADR records the llama.cpp binding and the catalogue. Errors: as stated. [paraphrase]

## Boundaries

- No meeting analysis prompts (S-26) beyond the `analysis_model` key. [paraphrase]
- No fine-tuned polish models, GGUF conversion or eval harness (S-37). [paraphrase]
- No GUI (the first-run Models step consumes `llm.models.*`). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Omarchy machines rarely run Ollama; a local engine on the same Vulkan backend makes Enhanced work out of the box, the way macOS ships a local model by default. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** a third ggml engine on the same backend with idle unload that returns the VRAM. [strategy:Local engines and performance]
- **Contract parity and agent surfaces:** the macOS model catalogue in GGUF with the same defaults. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
