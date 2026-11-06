# Enhanced refines while you talk

## Goal & Context

Enhanced mode adds 243-725 ms median and 1.2-3.0 s p95 after release (`docs/reports/polish-eval/README.md`), on top of transcription. That cost can be hidden. While the key is held, refine each transcript so far in the background and keep the result as a draft. At release, check the draft against the final prompt in large blocks and decode only from the first change, which is lossless under greedy decoding. When the final transcript matches the last draft's prompt, no forward pass runs at all. Published measurements of this technique report key-up to corrected text of about 31 ms p50 / 130 ms p90, against 221 / 681 ms without it, with most dictations needing no forward pass at release.

This spec brings that to Dettivo once Enhanced is worth turning on, so a rewrite costs almost nothing at release.

Origin: Gordon, 2026-11-06: a follow-up for later.

## Preconditions

- Enhanced passes its polish-eval gate on at least one model. Until then a faster Enhanced has no users.
- The input mismatch is settled. Enhanced sends the model the raw-layer text and uses the deterministic Polish text only as a draft in the repair prompt (`crates/dettivo-language/src/enhanced/pipeline.rs:238-239`), while `docs/polish.md:3` says it sends the cleaned text. A background refine needs one defined input on every pass, so the code or the doc is fixed first.
- fn-92 (dictation decodes while the key is held) has landed, because it supplies the transcript so far.

## Approach

- Add sessions to `dettivo-engine-llm`'s protocol, which is request-at-a-time today (ADR 0026): open, roll back to a token position, and forward a block returning the greedy choice for each position.
- During a hold, refine each new transcript in the background and keep the draft and its KV state. At release, verify the draft against the final prompt and decode only from the first mismatch.
- Prototype on Qwen3, whose attention-only layers roll back trivially. Models with convolution layers need that state kept separately.
- Prompt-lookup drafts taken from the transcript itself are the first speed-up. A learned drafter is out of scope.

## Acceptance

- **R1:** The final Enhanced text equals a one-shot refine of the same final transcript with the same model and settings, on every polish-eval row, under greedy decoding.
- **R2:** On the reference GPU, Enhanced's added time after release is reported at p50 and p90 through the daemon, and p50 is at most 25% of today's on takes of polish-eval length.
- **R3:** Capture and transcription never wait on a background refine; the GPU busy share during a hold is reported.
- **R4:** An ADR records the session protocol and the numbers; `docs/polish.md` follows; a changelog entry is added.

## Boundaries

- Enhanced stays off by default unless a separate decision changes that.
- No learned drafter and no fused-kernel work.

## Quick commands

- `cargo test -p dettivo-language -p dettivo-engine-llm -p dettivod`
- `dettivo-qa polish-eval`
- `just build test lint`
