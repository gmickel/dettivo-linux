---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-89-whisper-vocabulary-without-false.1 Implement Whisper vocabulary without false insertions

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Whisper now checks every listed term in a prompted dictation transcript. An echo-shaped run of terms, or a term whose mean token log probability is under -0.3, is settled against the same take decoded without the prompt; that second decode runs only when a term needs it (ADR 0095). The output guards moved to dettivo-transcribe, and `dettivo-qa bench vocabulary` now passes Whisper the terms and guards each clip as dictation does (ADR 0096).

Measured on the padded, guarded bench (Whisper large-v3-turbo, release Vulkan build, RTX 4090, shared GPU):
- Baseline at e0071ee, vocabulary on: 10/15 recall, 4 false insertions (distractor-omar x2, distractor-clay-name, distractor-omar-he), guard WER 0.000. Off: 2/15, 0, 0.011.
- Shipped -0.3 at 60efadb: 10/15, 0 false insertions, guard WER 0.000 (R1, R2).
- Sweep of 10 levels, one build each: -0.45 to -0.2 all give 10/15 and 0 insertions; -0.15 loses target-hyprland-1, -0.5 keeps the Clef distractor. Margins: 0.12 nats above target-hyprland-1 (-0.177) and 0.16 below distractor-clay-name (-0.458).
- R3: all 33 term-free clips were byte-identical to the baseline, with no second decode.
- R4: per-clip times are in the report JSON. Median over the 13 clips with a candidate: 0.0 ms. The 3 clips that needed the second decode took 78.0 to 84.5 ms.
- R5: ADR 0095 and 0096, docs/engines.md, docs/dictation.md, docs/qa.md, a CHANGELOG [Unreleased] entry, and docs/reports/benchmarks/vocabulary-2026-11-02-thor-whisper-threshold.md with its JSON files.

Tests: check_tests.rs covers no-term/no-decode (R3), confident term, unsure term, echo shape, prompt-free agreement, overlapping gaps (review regression) and token/text mismatch. gate.rs covers the session sending Whisper its prompt terms. The bench tests cover the Whisper args and the check-line parser.

Decisions:
- Only terms are checked, not aliases: the prompt carries terms only (ADR 0086), and checking aliases would decode twice on term-free takes, against R3.
- A confident term inside a sentence skips the second decode, because its outcome would be "keep" either way.
- A segment the check rewrote returns no word timings.

Follow-ups (not done):
- File transcription, re-runs and meetings still prompt Whisper without the check.
- A take that is only one listed name is echo-shaped. It pastes the prompt-free reading unless that reading also has the name.
- The Parakeet bench rows were not re-run (engine not built in this worktree). The Parakeet code path is unchanged.
- `cargo clippy -p dettivo-engine-whisper --features vulkan -- -D warnings` fails on the existing `unsafe` block in engine.rs (`vulkan_devices`). This predates the change; `just lint` does not build that feature.
- The fresh worktree needed `cargo build -p dettivod` before dettivo-qa's polish_eval test passes. This is environmental.
- No route to a mapped feature changed.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran [round 1: 3 reviewers gpt-6.1-sol NEEDS_WORK (overlapping-gap panic P1, report wording P3) -> fix 8412709 -> re-review SHIP]

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 60efadb890e7ecfd28936d4c11161185396b87a4, 2618cc980656668df74cbe889e253be468971db8, 841270997d2ecd94a7ab9812d5e0c8b22438d55c
- Tests: cargo test --no-fail-fast -p dettivo-session -p dettivo-engine-whisper -p dettivo-qa -p dettivo-transcribe -p dettivo-engine-proto (313 passed), cargo clippy (changed crates) -D warnings, dettivo-qa bench vocabulary --engines target/release (release Vulkan, 60efadb: whisper on 10/15, 0 false insertions, guard WER 0.000), threshold sweep: 10 release builds -1.0..-0.1 through the bench
- PRs: