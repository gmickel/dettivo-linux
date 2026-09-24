# Polish models

The production app keeps experiment selection under Advanced settings. Local experiments use the manifest and supported GGUF conversion workflow below. A model filename alone does not establish compatibility. Ollama and OpenAI-compatible providers use their own configured model names and the existing endpoint trust rules.


Enhanced on Linux runs on the same models, the same prompt profiles and the same guards as on macOS, and this page is how you prove it: how to bring the private dictation fine-tune over from the macOS training pipeline as a GGUF, how to switch Enhanced onto it with one key while the catalogue default stays put, how to score any candidate on the held-out sets with the macOS metrics and hard gates through the Linux engine, and how to read the report a run leaves behind ([ADR 0032](adr/0032-polish-fine-tune-sideload-and-the-eval-harness.md)).

## The models you can run

The catalogue ships four Qwen3 GGUFs and `qwen3-4b-instruct-2507` is the default ([docs/models.md](models.md), ADR 0026). A fine-tune never enters the catalogue: it is private, it arrives on this machine by hand, and the daemon loads it from a manifest, a *sideload*. Two forms are accepted:

- a **fused GGUF**: one `.gguf` file that loads on its own, the shape a fused Hugging Face export converts to;
- a **GGUF LoRA adapter** (`gguf-lora`): one adapter `.gguf` the engine applies over a catalogue base at load, so a 40 MB adapter rides on the 1.1 GB `qwen3-1.7b` the catalogue already downloaded.

Whichever form, Enhanced stays on the catalogue model until you opt in, and a sideload that cannot be resolved leaves Enhanced exactly where it was.

## Converting a fine-tune

`scripts/models/convert-polish-finetune.sh` is the runbook. It runs llama.cpp's own converters at the revision `llama-cpp-2` vendors (`e79e4bf6…`, pinned in the script), so the file the engine reads was written by the same code that reads it. Every step runs in a staging directory beside the sideload and only a complete, checksummed (and with `--verify`, loaded) model replaces the previous one and its manifest, so re-running an experiment id that fails halfway leaves the model the daemon loads as it was.

```
# a fused export (config.json + *.safetensors), quantised to Q4_K_M:
scripts/models/convert-polish-finetune.sh --source ~/exports/qwen3-1.7b-rewrite-fused \
    --id qwen3-1.7b-private-best --name "Current 1.7B Experiment" --setup-venv --verify

# a PEFT adapter over the catalogue 1.7B:
scripts/models/convert-polish-finetune.sh --adapter ~/exports/qwen3-1.7b-rewrite-lora \
    --base-hf Qwen/Qwen3-1.7B --base qwen3-1.7b --id qwen3-1.7b-private-lora --setup-venv --verify
```

The script names every step as it runs (`toolchain`, `source`, `checkout`, `python`, `quantizer`, `convert`, `quantize`, `checksum`, `manifest`, `verify`) and stops at the first that fails, naming it and the exit code. `--setup-venv` creates the interpreter it needs (torch on the CPU, transformers, llama.cpp's `gguf` package) under `$XDG_CACHE_HOME/dettivo/polish-convert-venv` with `uv` when it is installed and `python -m venv` otherwise; `--python <interpreter>` uses one you already have. `--dry-run` checks the toolchain and the paths and prints the plan without converting anything, which is what CI runs against `scripts/models/fixtures/hf-checkpoint` (`just polish-convert-check`). `--verify` loads the result in `dettivo-engine-llm --prompt` on the CPU and prints its answer to one dictated sentence.

What lands under the experiments directory (`$XDG_DATA_HOME/dettivo/models/polish-experiments` unless `[llm] experiments_dir` says otherwise):

```
polish-experiments/
  current.json                                   # the manifest
  qwen3-1.7b-private-best/
    qwen3-1.7b-private-best-Q4_K_M.gguf          # exactly one .gguf per directory
    qwen3-1.7b-private-best-Q4_K_M.gguf.sha256
```

`current.json` is the macOS manifest shape, so a manifest the macOS packaging script wrote reads as is:

```json
{ "id": "qwen3-1.7b-private-best", "displayName": "Current 1.7B Experiment", "relativeModelPath": "qwen3-1.7b-private-best" }
```

An adapter adds the two Linux fields, `"format": "gguf-lora"` and `"baseModel": "qwen3-1.7b"`. A manifest with neither is a fused GGUF. `relativeModelPath` is resolved under the experiments directory and must stay inside it; the directory it names holds exactly one `.gguf`, and a directory with none or two is reported rather than guessed at.

## Switching Enhanced onto it

```
dettivo llm experiment use current       # [llm] polish_experiment = "current"
dettivo llm experiment status
dettivo llm experiment clear             # back to [llm] model
```

`use` writes `[llm] polish_experiment` through the daemon's config path (the file keeps its comments) and the daemon binds the sideload at once: `llm.models.status` and `dettivo llm status` gain a row with `source = "sideload"` carrying the selection mark, `llm.providers.list` names it as the local provider's model, `polish.test` and every Enhanced dictation name it in `model`, and for an adapter `llm.engine.status` reports the `lora` file once loaded. The catalogue rows lose the selection mark while a sideload is in force, and `[llm] model` is untouched, so `clear` is all it takes to go back. `dettivo llm download`, `cancel` and `delete` never know a sideload's id: they answer `NOT_FOUND` naming the catalogue's ids, and nothing here is ever offered for download.

When the manifest does not resolve (no file, a directory with two GGUFs, a `gguf-lora` without `baseModel`, a path that escapes the directory), the row is named after the manifest with `manifest_error` saying which, `dettivo doctor` prints the same line under `llm`, and Enhanced runs on `[llm] model` with no notice to the user. An adapter whose base is not downloaded is `missing` naming the base and its `dettivo llm download --model <base>` command.

## Scoring a candidate

`dettivo-qa polish-eval` is the macOS bench and scorer on Linux. Every row of a held-out set goes through a daemon of the harness's own, `polish.test` in `enhanced` mode against the local engine, so what is scored is what the product would have inserted: the prompt profile, the guards, the repair pass and the fallback are all in the loop. The scorer is `score_results.py` ported case for case, the gate is `check_promotion.py`, and the targets are the macOS promotion targets checked in as `qa/polish-eval/targets.json`.

```
just polish-eval ~/private/eval-private-whisper-heldout-v1.jsonl qwen3-4b-instruct-2507
just polish-eval ~/private/eval-private-whisper-heldout-v1.jsonl sideload \
    --incumbent docs/reports/polish-eval/2026-09-05-qwen3-4b-instruct-2507-eval-private-whisper-heldout-v1.json
```

`--model` takes a catalogue id, `sideload` (the `current` manifest) or `sideload:<name>`, a GGUF path (run as a sideload of its own), and for a run without a model `fixture:<dir>` or `echo` (the QA mock provider). The held-out sets are read where they are with `--set` and never enter this repository; the rows' shape is the macOS canonical one (`id`, `split`, `input`, `gold`, `preset`, `style`, `language`, `app_bundle_id`, `protected_spans`, `required_fragments`, `forbidden_fragments`, `structure_fragments`). `--split` picks the rows (`heldout` by default, `all` for every row), `--cpu` pins the engine, `--engine-only` adds the raw generation figure (the rows through `dettivo-engine-llm --prompt --raw`, no guards: tokens per second, the `polish_bench` number), `--incumbent <report>` turns on the relative gates, and `--outputs <file>` writes every row's output to a file of yours for reading, outside the repository.

The metrics are the macOS ten: exact match, protected-token preservation, required-fragment retention, forbidden-fragment violation, structure retention, language-anchor retention, guard rejection, fallback used, median and p95 wall time, plus cuts by preset and by language. A row without `gold` is scored on the gates alone. A row with an unknown preset or style fails the run naming the row, and a run that ran fewer rows than the set fails with the count.

## Reading a report

A run writes `docs/reports/polish-eval/<date>-<model>-<set>.json` and regenerates the `README.md` table beside it (the latest report per model and set with its gate result). A report carries the metrics and the cuts, the model id with the file it loaded and that file's SHA-256, the backend, the engine version, the git sha, the row count and the row ids, the gate and the macOS comparison. It never carries a row's text; `crates/dettivo-qa/tests/polish_eval.rs` scans the directory for that on every test run.

The gate section states every hard gate as pass or fail with the value and the rule. `absolute_passed` reports those absolute gates. With `--incumbent`, relative promotion also requires the same selected sample content, split, ordered row IDs, scoring rules, targets, runtime binaries, runner, host, execution switches and backend. Reports carry content-free SHA-256 comparison fingerprints. A legacy or incompatible report sets `relative_skipped`, adds a failure and leaves `passed = false`, even when the absolute gates passed. A fresh incumbent run supplies the missing evidence. Compatible candidates must match or beat every hard-gate metric and improve a latency metric. `macos_baseline` retains its separate quality comparison; its latency rows remain informational because the runtimes differ (ADR 0055).

A candidate that clears every gate is still not promoted by anything here. Promotion is a config change recorded beside the report, `[llm] polish_experiment` on the machines that opt in, and the catalogue default does not move (the macOS optional-first rule). The stop rule in the targets file is the macOS one too: the confirmed 1.7B incumbent stays frozen until a challenger repeat-confirms a win.

In CI, `just qa-polish-eval` runs the twelve synthetic rows of `crates/dettivo-qa/fixtures/polish-eval/sample.jsonl` with the recorded rewrites under `rewrites/` (two rows staged to fail: one the guard rejects, one that answers the dictation) and compares the metrics with `golden.json`, failing on the first metric that differs. The dictionary-correction eval lives with the layer it measures: `cargo test -p dettivo-language --test dictionary_eval` scores the twenty public rows of the macOS `eval-v1.jsonl` through `dettivo-language::raw` against `docs/reports/polish-eval/dictionary-eval-v1.json`, which records the Linux figures beside the macOS `exact` and `hybrid` strategies' and names every row that differs.

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[llm] polish_experiment` | `""` | The manifest name Enhanced runs on (`current` reads `current.json`); empty keeps `[llm] model`. |
| `[llm] experiments_dir` | `""` | Where manifests and fine-tunes live; empty means `<models_dir>/polish-experiments`. |
| `[llm] model` | `"qwen3-4b-instruct-2507"` | The catalogue default; a sideload never changes it. |

[docs/config.md](config.md) has every key; [docs/engines.md](engines.md) has the engine's `lora` load field and the CLI mode's `--lora`; [docs/api/linux-deltas.md](api/linux-deltas.md) registers the `source`, `format`, `base_model` and `manifest_error` fields of `llm.models.status`, `polish_experiment` on its result and `lora` on `llm.engine.status`.
