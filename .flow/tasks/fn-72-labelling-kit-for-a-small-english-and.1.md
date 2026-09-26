---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-72-labelling-kit-for-a-small-english-and.1 Implement Labelling kit for a small English and German meeting set

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`just diar-label <alias>` opens a keyboard-driven labelling page on 127.0.0.1 over a retained meeting's 15-minute excerpt with the most speaker changes. Drafts blend both engines' cached outputs, disagreements come first, and finished labels land in `<eval>/labels/` in the bench's format, where `just diar-bench` reports `labelled-en`/`labelled-de` with the WDER row. The protocol lives in docs/diarization-labelling.md.

- R1: page with Enter/1-9/0/n/r/Space/Tab keys, autoplay per line from its own track, 1.5x speed, time-order context strip. On the ten retained meetings a 15-minute excerpt is 299-401 lines, 87-97% of them agreed by both engines. The about-20-minutes target is untimed and the doc says so.
- R2: labels round-trip through `data.reference_units` (test), and a real `just diar-bench` run on a temp eval dir reported labelled-de. The bench now labels the wrong-speaker row WDER and counts a labelled excerpt's own minutes.
- R3: `blend` refuses fewer than two engines (test). The page hides which engine proposed which letter. The draft is recorded as `blend:current+nemotron` or `blank` in the page, progress and labels files.
- R4: server bound to a hard-coded 127.0.0.1, with a per-run secret path, a Host check and a CSP with no external source. Tests cover the bind, token and Host rejection, and a page with no external URL. Files are mode 600 under the eval dir, and nothing enters git.
- R5: docs/diarization-labelling.md (overlap, backchannels, unknown, mic side, names), linked from docs/diarization-bench.md and docs/qa.md.

Gate: `just build test lint` green on 59c78e1, run as `just test-rust` plus the other recipes. Two earlier full runs failed only timing-bound dettivod tests at load 27-59 from concurrent worktrees.
Leftover test data (confidential, mode 700): /home/gordon/.cache/dettivo-qa-tmp/fn72/eval holds DE-2/DE-3 test sessions, a synthetic DE-3 labels file and a run. The dcg guard blocked removing it, so Gordon needs to delete it by hand.

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: 59c78e156bd507f5c43d85d3f82d587d02d44886
- Tests: baseline: green (just build test lint, pre-edit, exit 0), python3 scripts/qa/test_diarlabel.py (7 tests ok; red-checked: HOST=0.0.0.0 and a one-engine blend both fail it), just build test-qt test-diarization-score lint (exit 0, HEAD 59c78e1), just test-rust (cargo test --workspace, exit 0, HEAD 59c78e1), INCONCLUSIVE then green: two earlier full runs of just build test lint failed only timing-bound dettivod tests (llm_local an_enhanced_dictation_inserts_the_local_rewrite; meetings_retry_cancel) at load 27-59 from concurrent worktrees; no Rust changed; test-rust rerun green, manual: DETTIVO_EVAL_DIR=<temp copy> just diar-label DE-3 driven in chromium (keys, autoplay audio, save, completion) then just diar-bench reported labelled-de with WDER, 15 min
- PRs: