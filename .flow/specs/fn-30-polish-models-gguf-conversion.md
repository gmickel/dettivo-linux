# Polish models: GGUF conversion, the sideloaded fine-tune, the eval harness port and the hard-gate report

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-37 row: "`polish-models-gguf-conversion-finetune-eval-parity` | phase 2 | depends on S-33, S-14 | FR-M8 to M11, Principle 14; conversion runbook (fused export to GGUF, LoRA to GGUF), sideload manifest, eval harness port, hard-gate report, optional-first rollout | Held-out eval report through `dettivo-engine-llm` for the 4B default and the tuned 1.7B; gate comparison against the macOS baseline; sideload manifest test"
> masterplan FR-M8: "The Enhanced and meeting-analysis models are the macOS catalogue in GGUF form ... Prompt profiles, presets, styles and the deterministic layers ... are ported verbatim from the macOS repository and covered by the same golden cases."
> masterplan FR-M9: "The private dictation polish fine-tune (the tuned 1.7B challenger from the macOS fine-tune pipeline) runs on Linux through the local engine after conversion from its fused Hugging Face export to GGUF with llama.cpp's converter, or as a GGUF LoRA adapter over the base model. It follows the macOS rollout rules: optional and opt-in first, the 4B runtime stays default, promotion only after clearing the same hard gates (protected-token preservation, required-fragment retention, forbidden-fragment violation, language-anchor retention, structure retention, guard rejection rate, fallback-used rate, median and p95 latency) on the same held-out sets, measured through the Linux engine, not the MLX runtime."
> masterplan FR-M10: "Fine-tune artifacts, training data, benchmark outputs and operator notes stay private as on macOS. Linux loads a tuned model from a sideload manifest at `$XDG_DATA_HOME/dettivo/models/polish-experiments/current.json` with the macOS manifest shape, and the catalogue never lists a public download for it unless decision D14 changes that."
> masterplan FR-M11: "The macOS polish evaluation harness (the `polish_eval` and `polish_bench` scripts, the held-out JSONL sets for Whisper and Parakeet transcripts, and the dictionary-correction eval) is ported to run against `dettivo-engine-llm --prompt` so every candidate model is scored the same way on both platforms and results are comparable per release."
> masterplan Principle 14: "Same models, same bar. Polish and Enhanced use the same model catalogue, prompt profiles, deterministic layers, private fine-tunes, evaluation sets and hard gates as macOS. Linux converts formats; it does not fork quality."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Enhanced on Linux is judged by the macOS yardstick. The catalogue's four Qwen3 GGUFs and the polish layers are already in place (ADR 0023, ADR 0026); this spec adds the three things that make quality comparable rather than asserted. A sideload path loads the private 1.7B fine-tune from `$XDG_DATA_HOME/dettivo/models/polish-experiments/current.json` in the macOS manifest shape, as a fused GGUF or a GGUF LoRA adapter over the catalogue base, opt-in through one config key while `qwen3-4b-instruct-2507` stays the default. `dettivo-qa polish-eval` ports the macOS bench and scorer: every row of a held-out JSONL set goes through the daemon's own Enhanced pipeline against the local engine, the ported scorer computes the macOS metrics, and the hard-gate check compares the candidate with the incumbent and with the macOS baseline numbers, writing a report under `docs/reports/polish-eval/`. A documented runbook converts the macOS fused export or adapter to GGUF with llama.cpp's converters and quantises it to the catalogue footprint. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Sideload (`crates/dettivo-speech/src/llm/sideload.rs`): reads `current.json` (`id`, `displayName`, `relativeModelPath`, the macOS fields; Linux accepts two optional additions, `format` = `gguf` or `gguf-lora` (default `gguf`) and `baseModel` (a catalogue id, required for `gguf-lora`)), resolves `relativeModelPath` under `polish-experiments/` to a directory holding exactly one `.gguf` (or one adapter `.gguf` for the LoRA form), and exposes it as a model entry with `source = "sideload"` in `llm.models.status` and `dettivo llm status` (never in the catalogue file, never downloadable). `LocalLlm` binds `[llm] polish_experiment` to that entry when set; the engine's `load` gains an optional `lora` path that llama.cpp applies over the base at load. A manifest that does not resolve is reported by name in `llm.models.status` and `dettivo doctor`, and Enhanced runs on the default model. [inferred]
- Eval harness (`crates/dettivo-qa/src/polish_eval/`): `dataset` reads the macOS canonical JSONL rows (`sample` with `input`, `expected`, `preset`, `style`, `bundle_id`, `language`, `protected_tokens`, `required_fragments`, `forbidden_fragments`, `split`); `runner` starts an isolated daemon the way `contract` does, with `[llm] provider = "local"` and the candidate model (a catalogue id, `sideload`, or a GGUF path), and drives every row through `polish.test { mode: enhanced, preset, style, bundle_id }` so the prompt profile, the guards, the repair pass and the fallback are the product's; `scorer` is `score_results.py` ported case for case (protected-token preservation, required-fragment retention, forbidden-fragment violation, language-anchor retention, structure retention, exact match, guard rejection, fallback used, median and p95 wall time); `gate` is `check_promotion.py` ported: absolute hard gates from `qa/polish-eval/targets.json` (the macOS `promotion-targets` values) and the relative gates against an incumbent report. `--engine-only` runs the rows through `dettivo-engine-llm --prompt --raw` for tokens per second without the guards, the `polish_bench` number. The dictionary-correction eval reads the macOS `eval-v1.jsonl` shape and scores `dettivo-language::raw` directly. [inferred]
- Fixtures: `crates/dettivo-qa/fixtures/polish-eval/sample.jsonl` (twelve synthetic rows in the canonical shape, no private text) plus a recorded `DETTIVO_MOCK_LLM=fixture:` directory, so CI exercises the dataset reader, the runner, the scorer and the gate without a model; the private held-out sets (`eval-private-whisper-heldout-v1.jsonl`, `eval-private-parakeet-heldout-v1.jsonl`, `eval-priority-heldout-v1.jsonl`) are read from `--set <path>` outside the repository and never committed. [paraphrase]
- Reports: `docs/reports/polish-eval/<date>-<model>-<set>.json` carries the metrics, the model id and checksum, the backend, the engine version, the row count and the git sha, with a `README.md` table (latest per model and set, gate result); the macOS baseline numbers for the same sets are checked in as `docs/reports/polish-eval/macos-baseline.json` (metrics only, no rows). [inferred]
- Runbook: `docs/polish-models.md` and `scripts/models/convert-polish-finetune.sh` (a fused Hugging Face export through llama.cpp's `convert_hf_to_gguf.py`, an adapter through `convert_lora_to_gguf.py`, quantisation to `Q4_K_M` with `llama-quantize`, the SHA-256, the directory layout and the `current.json` to write); the script pins the llama.cpp revision `llama-cpp-2` vendors so the converter matches the engine. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Config: `[llm] polish_experiment = ""` (empty: off; `current` reads `current.json`; the 4B default stays `[llm] model`), `[llm] experiments_dir = ""` (empty: `$XDG_DATA_HOME/dettivo/models/polish-experiments`); documented in `docs/config.md` and printed by `config print-default`. [inferred]
- `llm.models.status` rows gain `source` (`catalogue` or `sideload`) and, for a sideload, `format`, `base_model`, `manifest_error`; `llm.engine.status` reports `lora` when an adapter is loaded; both recorded in `docs/api/linux-deltas.md` with fixtures. [inferred]
- CLI: `dettivo llm experiment status|use|clear` (reads the manifest, sets or clears `[llm] polish_experiment`); `dettivo-qa polish-eval --set <jsonl> --model <id|sideload|path> [--split heldout] [--incumbent <report>] [--engine-only] [--dictionary <jsonl>] [--out <dir>] [--json]`; `just polish-eval set=<path> model=<id>`. [inferred]
- Engine protocol: `load` gains optional `lora` (a path); documented in `docs/engines.md`. [inferred]

## Edge Cases & Constraints

- The manifest names a directory with two GGUFs or none: the sideload is `manifest_error` naming the directory and Enhanced runs on the default model with no notice to the user. [inferred]
- `gguf-lora` whose base model is not downloaded: readiness is `missing` naming the base id and the download command. [inferred]
- A held-out set row without `expected`: scored on the gates alone, as the macOS scorer does; an unknown preset in a row fails the run naming the row. [paraphrase]
- The engine falls back to the CPU mid-run: latency metrics carry the backend, and the gate compares only reports from the same backend. [inferred]
- A candidate that clears the hard gates is still not promoted by code: promotion is a config change recorded with the report, and the catalogue default does not move (FR-M9). [paraphrase]
- Private rows never reach a report: the report stores metrics and row ids, never `input` or `output`. [paraphrase]

## Acceptance Criteria

- **R1:** The sideload resolver is unit-tested against the macOS manifest shape and both Linux forms (fused GGUF, LoRA over `qwen3-1.7b`), `llm.models.status` lists the sideload with `source = "sideload"` and never in `dettivo llm download`'s ids, and an Enhanced dictation with `[llm] polish_experiment = "current"` on this machine runs on the tuned model (the `polish.test` result names it). Errors: an unresolvable manifest reports `manifest_error` by name and Enhanced runs on `[llm] model`. [paraphrase]
- **R2:** `dettivo-qa polish-eval` on the sample set with the fixture LLM in CI produces a report whose metrics match a checked-in golden, and the scorer and gate are unit-tested against cases ported from `score_results.py` and `check_promotion.py` (each macOS metric with one passing and one failing row). Errors: a metric that differs from the golden fails naming it. [paraphrase]
- **R3:** On this machine the held-out Whisper and Parakeet sets run through `qwen3-4b-instruct-2507` and the sideloaded 1.7B on Vulkan, and `docs/reports/polish-eval/` gains both reports plus the `macos-baseline.json` comparison with every hard gate stated as pass or fail; the reports carry no row text (a test scans them). Errors: a run with fewer rows than the set fails with the count. [paraphrase]
- **R4:** The dictionary-correction eval scores the macOS `eval-v1.jsonl` shape through `dettivo-language::raw` and reports the same accuracy figure the macOS run records for the shared public rows. Errors: a row whose correction differs from macOS fails naming it. [paraphrase]
- **R5:** `scripts/models/convert-polish-finetune.sh` converts a small public Qwen3 checkpoint in CI (`--dry-run` validates the toolchain and paths; the full conversion runs on this machine) into a GGUF the engine loads in CLI mode with `--prompt`, and writes a `current.json` the resolver accepts. Errors: a converter or quantiser exit code fails the script naming the step. [inferred]
- **R6:** `docs/polish-models.md` (the runbook, the manifest, the opt-in key, the eval command, how to read a report), `docs/config.md`, `docs/engines.md` for `lora`, `docs/api/linux-deltas.md` for the new fields, and an ADR recording the sideload path, the eval harness port and the optional-first rollout rule. Errors: `just docs` fails on a missing key or an unindexed ADR. [paraphrase]

## Boundaries

- No fine-tuning pipeline on Linux; training stays in the macOS repository and only its exports arrive here. [paraphrase]
- No public download of the tuned model and no catalogue entry for it (D14). [paraphrase]
- No change to the polish layers or the goldens (S-14); no meeting analysis prompts (S-26). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The fine-tune is the product's edge on dictation quality, and it earns its place on Linux only when the same sets and the same gates say so through the Linux engine. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** the tuned model and the 4B default are measured through llama.cpp on the user's GPU with the macOS gates. [strategy:Local engines and performance]
- **Contract parity and agent surfaces:** the sideload fields and the report shape are registered deltas an agent can read. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
