---
satisfies: [R1, R2, R3, R4]
---
# fn-94-enhanced-rejects-rewrites-of.1 Implement Enhanced rejects rewrites of implausible length

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Enhanced's output guard gains the length rules it lacked, and local rewrites get an output budget.

- R1: compared the three rules with `guard.rs`. Added: a word-count expansion rule (>2x words; the old character cap skipped dictations under 24 characters), a collapse rule (<0.4x words past 8, technical input exempt; the old contraction rule passed any rewrite ending a sentence), a short-deletion rule against the deterministic draft (1-2 meaningful words dropped, kept words in order, fillers/hedges and adjacent repeats allowed; nothing caught a dropped `not` before), and the output budget (none existed; every request asked for 1024 tokens). Tests in `guard::length`, `enhanced::tests`, and `dettivo-engine-llm` `a_generation_that_would_run_on_stops_at_max_tokens` (real model).
- R2/R3: `dettivo-qa polish-eval`, qwen3-4b-instruct-2507 and qwen3-1.7b, two private held-out sets plus the 275-row corpus v3, baseline run twice. Protected, required and forbidden unchanged everywhere; guard and fallback rates unchanged; every median at or below both baseline runs (4B corpus 375/367 to 344 ms). The budget fired on none of the 570 rows. Raw results in ~/.cache/agent-work/fn94.
- R4: `docs/polish.md` length-rules section, ADR 0103, changelog entry, ADR index row; held-out reports under docs/reports/polish-eval.

stage: impl-review - ran [round 1 NEEDS_WORK (3 lenses: repeat-exemption bypass, order-insensitive match, repair passes on short deletion) .. round 2 SHIP] (model: gpt-6.1-sol)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 3b5c33d473bfeb218fd0939c74a3e702266fd673, aa697a1d6c3c7e6c665933b1d910b9f4677f3ae3, 942e60d45b68063094fdd15126cd32bf496b15ee
- Tests: cargo test -p dettivo-language -p dettivo-engine-llm, cargo test -p dettivod --bin dettivod llm_engine, cargo clippy (deny warnings) on dettivo-language, dettivo-engine-llm, dettivod, dettivo-qa polish-eval (4B and 1.7B; held-out whisper/parakeet; corpus v3 split all)
- PRs: