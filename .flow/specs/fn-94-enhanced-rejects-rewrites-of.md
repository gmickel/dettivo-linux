# Enhanced rejects rewrites of implausible length

## Goal & Context

Enhanced mode sends a take to a local language model and keeps the deterministic Polish text whenever the model is slow, missing or wrong (`docs/polish.md`). The guard in `crates/dettivo-language/src/enhanced/guard.rs` already rejects refusals, fences, transcript markers, dropped protected tokens and numbers, lost question marks, and some contractions and expansions. Three simple rules catch whole classes of model failure that it may not cover:

- an output more than 2x the input's word count, or under 0.4x once the input passes 8 words, falls back to the transcript;
- generation stops at an output budget of 1.5x the input tokens plus 32;
- a short deletion (2 words or fewer) that removes only meaningful words is rejected.

This spec adds whichever of these the guard does not already cover, so a runaway or collapsing rewrite never reaches the user and never costs more time than it should.

Origin: Gordon, 2026-11-06.

## Approach

- Compare the three rules with `guard.rs` one by one and list the gaps, with an example input for each.
- Add only the missing rules, plus the output budget on Enhanced's requests to `dettivo-engine-llm` if none exists.
- Score with `dettivo-qa polish-eval` before and after.

## Acceptance

- **R1:** For each of the three rules, a test shows Dettivo rejects its failure case, by an existing or a new rule; the ADR lists which rules were added.
- **R2:** `dettivo-qa polish-eval` shows no regression in the protected, required and forbidden metrics, and reports the guard and fallback rates before and after.
- **R3:** If an output budget is added, a runaway generation stops at the budget and falls back, and the median Enhanced latency on polish-eval does not rise.
- **R4:** `docs/polish.md` describes the guard's length rules; ADR 0032 or a new ADR records the change; a changelog entry is added.

## Boundaries

- Enhanced stays off by default (`mode = "raw"`).
- Raw and deterministic Polish output is unchanged.

## Quick commands

- `cargo test -p dettivo-language -p dettivo-engine-llm`
- `dettivo-qa polish-eval`
- `just build test lint`
