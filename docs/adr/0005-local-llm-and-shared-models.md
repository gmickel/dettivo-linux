# 0005. A local language model in v1, with the same models and fine-tunes as macOS

Status: Accepted 2026-09-03, amended 2026-09-06 by 0049 (no engine loads a catalogue model that did not verify, and a manifest names every file)

## What this gives you

Enhanced dictation and meeting analysis work after one optional download, with no Ollama or other service to install, and the rewrite quality is the same you get on the Mac because the models, prompts, deterministic layers and evaluation gates are the same.

## Situation

macOS ships an in-process MLX model by default. Windows requires Ollama. Omarchy machines rarely run Ollama (the development machine does not). With engine processes in place (ADR 0003) and `llama-cpp-2` tracking upstream (0.1.156, 2 September 2026), a local engine is incremental work on the same Vulkan backend. The macOS repository holds the polish fine-tune contract, the held-out evaluation sets and the hard gates; forking any of it would fork quality.

## Decision

The LLM provider layer has three providers behind one interface: `local` (the llama.cpp engine with a catalogue GGUF, the default once downloaded), `ollama` (auto-detected on localhost) and `openai_compatible` (user configured, non-localhost endpoints require an explicit trust confirmation). The catalogue mirrors macOS in GGUF: Qwen3 4B Instruct 2507 as the default for Enhanced and meeting analysis, Qwen3 1.7B, 4B and 8B as options. The private tuned 1.7B converts from its fused export to GGUF, ships only through a sideload manifest, stays optional, and is promoted only after clearing the macOS hard gates on the same held-out sets measured through the Linux engine.

## Consequences

- Polish stays deterministic and needs no model at all; Enhanced degrades to Polish with a notice when no provider is available.
- First run offers the 2.5 GB download and lets the user skip it.
- The macOS `polish_eval` and `polish_bench` harness is ported to run against `dettivo-engine-llm --prompt`, so results compare per release.
- Fine-tune weights, training data and benchmark outputs stay private, as the macOS contract requires; the catalogue never lists a public download for them.
