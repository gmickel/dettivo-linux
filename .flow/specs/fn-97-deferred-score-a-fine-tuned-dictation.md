# Score a fine-tuned dictation refiner as an Enhanced sideload

**Deferred (Gordon, 2026-11-06): future work, not scheduled. Do not start this spec until Gordon reopens it.**

## Goal & Context

Enhanced mode runs Qwen3 GGUFs, and every polish-eval report so far fails the gate (`docs/reports/polish-eval/README.md`). Small open models fine-tuned specifically for rewriting dictation now exist, for example an LFM3-1.7B with a merged LoRA, published as a Q4_K_S GGUF of about 1 GB and trained on tens of thousands of dictation rewrites in several writing modes. Their published figures are mixed: judge preferences on lectures look strong, while real-dictation WER is no better than the previous generation's.

This spec measures such a refiner on Dettivo's own eval, so the decision rests on our numbers rather than an announcement.

Origin: Gordon, 2026-11-06: deferred future work.

## Approach

- Pick the candidate refiner when the spec is reopened and record its source, version and licence.
- Check that the llama.cpp vendored by `dettivo-engine-llm` (`llama-cpp-2 =0.1.156`) loads its architecture. If it does not, record the upgrade it needs and stop there.
- Load it through ADR 0032's sideload (`[llm] polish_experiment`) with a prompt profile in `enhanced/prompt.rs` matching the prompt format it was trained on.
- Run `dettivo-qa polish-eval` against Qwen3 1.7B and 4B on the same rows, plus a German held-out slice, since such refiners are often trained on English only.

## Acceptance

- **R1:** A polish-eval report compares the refiner with Qwen3 1.7B and 4B on the same rows: hard gates, latency p50/p95, and the German slice.
- **R2:** An ADR records the result and the decision: adopt it as a documented sideload, or not.
- **R3:** If it is adopted, `docs/polish.md` describes how to sideload it and states its licence.

## Boundaries

- Licence: LFM-based weights ship under the LFM Open License, which does not license commercial use by organisations with $10M or more in annual revenue. Such a model can only be a user-downloaded sideload (ADR 0065), never a default catalogue entry or part of a distro package (fn-87).
- The default mode stays `raw`.

## Quick commands

- `cargo test -p dettivo-language -p dettivo-engine-llm`
- `dettivo-qa polish-eval`
