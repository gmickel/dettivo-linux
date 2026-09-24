# Polish: deterministic transforms, the Enhanced provider layer and the polish.* API

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-14 row: "`polish-deterministic-enhanced-provider-layer` | phase 2 | depends on S-07 | FR-M1 to M7 except the local engine; `polish.*`; dictionary and replacements; Ollama and endpoint providers | `polish.*` fixtures; deterministic polish goldens; mock LLM echo and fixture tests; guard and fallback tests; remote endpoint trust test"
> masterplan FR-M1: "Raw returns engine text with dictionary correction, replacement table and spoken punctuation applied. Mode identifiers on the wire match macOS: `raw`, `deterministic_polish` (display name Polish), `enhanced`, `meeting`."
> masterplan FR-M2: "Polish is deterministic and needs no model: global transforms (`fixGrammar`, `removeFillers`, `smartPunctuation`), the preset's post-processors and the app profile's custom rules. It never calls the LLM layer."
> masterplan FR-M3: "Enhanced runs the Polish pass, then an LLM rewrite using the resolved preset, style and custom rules. If the provider is unavailable within the timeout, or the rewrite guard decides the deterministic output is already good ..., the Polish result is used and the notice says so."
> masterplan FR-M4: "Polish configuration follows the macOS `PolishRulesConfig` schema version 1: presets `email`, `code`, `chat`, `notes`, `generic`; styles `asDictated`, `formal`, `casual`, `veryCasual`; enabled transforms; custom rules up to 500 characters; app profiles keyed by app id; custom presets. `polish.rules.*`, `polish.presets.list`, `polish.test`, `polish.apps.list` and `polish.apps.set` are implemented per contract."
> masterplan FR-M5: "The effective policy is frozen at session start with a deterministic config hash for log correlation, and resolves in the macOS order: user app profile, default app mapping, app-class override ..., then global."
> masterplan FR-M6: "The LLM provider layer has three providers behind one interface: `local` ..., `ollama` (auto-detected on localhost through its tags endpoint ...) and `openai_compatible` (user-configured). Any non-localhost endpoint requires an explicit trust confirmation and is stored in an allowlist, matching the macOS privacy gate."
> masterplan FR-M7: "Thinking blocks are stripped from model output, empty or degenerate rewrites fall back to the Polish result, and the output guard rejects rewrites that drop code, paths or numbers present in the input."
> masterplan FR-M8: "Prompt profiles, presets, styles and the deterministic layers (protected-token preservation, `@path` insertion, marker stripping, no-touch spans, output guards, retry and fallback routing) are ported verbatim from the macOS repository and covered by the same golden cases."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Dictation gets its two other modes. `dettivo-language` becomes the text-processing crate: the raw layer the session already applies (dictionary, replacements, spoken punctuation) plus the deterministic Polish pass (the macOS transforms, presets, styles, custom rules and app profiles, ported with their golden cases) and the Enhanced pass (the Polish result rewritten by a language model through a provider interface with the macOS guards, retries and fallback). The provider layer ships `ollama` and `openai_compatible` now, with the non-localhost trust gate, and leaves a `local` slot the llama.cpp engine spec fills. The `polish.*` methods answer the contract so the GUI, CLI and MCP configure it the same way, and `dictation.start { mode: "deterministic_polish" | "enhanced" }` stops answering `NOT_IMPLEMENTED`. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/dettivo-language` (depends on `dettivo-proto`, `dettivo-core`): modules `raw` (the existing dictionary, replacements and spoken punctuation, moved here from the session crate), `polish` (transforms `fixGrammar`, `removeFillers`, `smartPunctuation`; presets `email`, `code`, `chat`, `notes`, `generic` with their post-processors; styles `asDictated`, `formal`, `casual`, `veryCasual`; custom rules up to 500 characters; custom presets), `policy` (the `PolishRulesConfig` schema version 1, resolution order user app profile, default app mapping, app-class override, global; the frozen effective policy with its config hash), `enhanced` (prompt profiles, protected-token preservation, `@path` insertion, marker stripping, no-touch spans, output guards, thinking-block stripping, retry and fallback routing, the rewrite guard that skips short generic text, plain commands and already formatted text), and `provider` (the `LlmProvider` trait: `rewrite(request) -> Result<Rewrite>` with a timeout, `probe() -> Availability`, `name`). The macOS golden cases (the polish and enhanced fixture sets from the macOS repository) are checked in under `crates/dettivo-language/tests/goldens/` and drive the tests verbatim. [paraphrase]
- Providers: `ollama` (auto-detected through `GET /api/tags` on `http://127.0.0.1:11434`, recommended models `qwen3:4b-instruct`, `qwen3:1.7b`, `qwen3:4b`, `qwen3:8b`, chat endpoint with the prompt profile), `openai_compatible` (base URL, model, key from the Secret Service or `[llm] api_key_file`, chat completions), `local` (a stub reporting `unavailable: the local engine arrives with its spec`; the engine spec replaces it). Any non-localhost endpoint needs an explicit trust confirmation: `llm.endpoints.trust { url }` (Linux addition) or `dettivo llm trust <url>`, stored in `[llm] trusted_endpoints`; an untrusted remote answers `CONFLICT` with `kind = endpointNotTrusted` and the rewrite falls back to Polish with the notice. `DETTIVO_MOCK_LLM=echo|fixture:<dir>` is the mock provider from the QA contract. [paraphrase]
- Session wiring: the pipeline's mode step calls `raw`, then `polish` for `deterministic_polish` and `enhanced`, then the provider for `enhanced` with the frozen policy; the notice (`fallback_used`, `guard_rejected`, `provider_unavailable`, reason) rides on the completion payload and the history item; the session freezes the policy and its hash at start and logs the hash. Timeouts: `[llm] timeout_ms` (default 8000), never blocking the insertion beyond it. [paraphrase]
- Config: `[polish]` (the schema version 1 shape serialised to TOML: `transforms`, `default_preset`, `default_style`, `custom_rules`, `apps` table keyed by app id, `presets` for custom presets) and `[llm]` (`provider = "auto" | "local" | "ollama" | "openai_compatible"`, `ollama_url`, `ollama_model`, `endpoint_url`, `endpoint_model`, `api_key_file`, `trusted_endpoints`, `timeout_ms`, `max_retries`). `polish.rules.set` writes through the daemon's config path so comments survive. [inferred]
- Capabilities: `system.capabilities.polish` (`modes`, `presets`, `styles`, `custom_presets`) and `llm` (`providers` with availability, `local_available = false` until the engine spec). [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `polish.rules.get`, `polish.rules.set`, `polish.rules.reset`, `polish.presets.list`, `polish.test { text, preset?, style?, bundle_id?, mode? } -> { raw, polished, enhanced?, notice?, policy_hash }`, `polish.apps.list`, `polish.apps.set { bundle_id, preset, style?, custom_rules? }` per the contract fixtures (Linux carries the Wayland app id or X11 class in `bundle_id`). [paraphrase]
- `dictation.start { mode }` accepts `deterministic_polish` and `enhanced`; the completion payload carries `mode`, `notice` and `policy_hash` (delta register). [paraphrase]
- Linux additions with fixtures: `llm.providers.list`, `llm.endpoints.trust`, `llm.endpoints.list`. [inferred]
- CLI: `dettivo polish test|rules|presets|apps ...`, `dettivo llm providers|trust <url>|test "<text>"`, `dettivo dictation start --mode deterministic_polish|enhanced`. [inferred]
- MCP later reads `polish.rules.*` and `polish.apps.set` (its own spec). [paraphrase]

## Edge Cases & Constraints

- Provider slower than the timeout: Polish result inserted, notice `provider_unavailable`, the rewrite is not awaited afterwards. [paraphrase]
- Rewrite drops a code span, a path or a number present in the input: guard rejects, Polish result used, notice `guard_rejected`. [paraphrase]
- Thinking blocks (`<think>…</think>`) in the output: stripped before the guard. [paraphrase]
- Empty or degenerate rewrite (shorter than a fraction of the input, repeated tokens): fallback. [paraphrase]
- Custom rule over 500 characters: `INVALID_PARAMS` naming the limit. [paraphrase]
- App profile for an app id with no preset: falls through the resolution order; the hash changes with any input to it. [paraphrase]
- Ollama present but the model missing: availability says so with the pull command. [inferred]

## Acceptance Criteria

- **R1:** The deterministic layers pass the macOS golden cases verbatim (transforms, presets, styles, custom rules, protected tokens, `@path` insertion, marker stripping, no-touch spans) in `dettivo-language` unit tests; the policy resolution order and the frozen hash are unit-tested with app profiles, default mappings and class overrides. Errors: an over-long custom rule is rejected naming the limit. [paraphrase]
- **R2:** Enhanced with the mock provider: `echo` returns the Polish text unchanged with no notice, the fixture provider replays recorded rewrites, the output guard rejects a fixture that drops a path and falls back with `guard_rejected`, a degenerate rewrite falls back, thinking blocks are stripped, and a provider timeout falls back with `provider_unavailable` inside `timeout_ms`. Errors: as stated. [paraphrase]
- **R3:** Every `polish.*` fixture passes against the live daemon; `polish.rules.set` round-trips through `config.toml` preserving comments; `polish.test` returns raw, polished and enhanced text with the policy hash; the Linux `llm.*` additions have fixtures and pass. Errors: unknown preset or style names are `INVALID_PARAMS` listing the valid ones. [paraphrase]
- **R4:** A dictation through the mock microphone in `deterministic_polish` mode inserts the polished text and in `enhanced` mode with `DETTIVO_MOCK_LLM=echo` inserts the polished text with the notice, the history item stores mode, notice and hash, and the completion event carries them. Errors: `enhanced` with no provider available inserts the Polish text with the notice. [paraphrase]
- **R5:** Providers: `ollama` is detected on a mock tags endpoint (a local test server) and rewrites through its chat endpoint; `openai_compatible` rewrites through a local mock server; a non-localhost endpoint is refused with `endpointNotTrusted` until `llm.endpoints.trust` stores it, after which it is used; the key comes from the Secret Service or the key file. Errors: as stated. [paraphrase]
- **R6:** `docs/polish.md` explains the three modes, the resolution order, the providers and the trust gate; `[polish]` and `[llm]` keys are documented and printed by `config print-default`; the CLI verbs are documented; an ADR records the provider layer and the trust gate. Errors: as stated. [paraphrase]

## Boundaries

- No local llama.cpp engine; the `local` provider is a documented stub until S-33. [paraphrase]
- No fine-tuned polish models, GGUF conversion or eval harness (S-37). [paraphrase]
- No settings GUI (S-19) and no MCP exposure (S-29). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Raw text is a demo; Polish is what people keep on, and Enhanced with Ollama is what Omarchy users with a GPU already have; porting the macOS layers verbatim with their goldens keeps parity honest. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the `polish.*` methods and mode identifiers match macOS byte for byte, with the goldens as proof. [strategy:Contract parity and agent surfaces]
- **Local engines and performance:** the provider layer keeps the rewrite local by default and gates anything remote behind explicit trust. [strategy:Local engines and performance]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
