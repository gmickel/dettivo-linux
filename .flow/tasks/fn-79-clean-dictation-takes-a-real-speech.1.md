---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8]
---
# fn-79-clean-dictation-takes-a-real-speech.1 Implement Clean dictation takes: a real speech gate and engine output guards

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Dictation takes pass a two-stage speech gate (loudest 30 ms window with DC removed at -50 dBFS, then Silero VAD at threshold 0.4 and 160 ms on the CPU, falling back to the level alone with a doctor row), are padded to 1.5 s with a 400 ms Parakeet tail and a second recognition when padding empties a take, and their text passes loop, hallucination and prompt-echo guards with a 500-character prompt cap; silence_peak_threshold is retired (ADR 0081). The capture ends at the stop request; per Gordon's 2026-11-01 decision, up to one PipeWire capture quantum before Cmd::Stop runs counts as inside the take for R1 (ADR 0082), and the mock microphone publishes nothing after the stop.

QA follow-up (088517b): `dettivo doctor` offers the silero-vad download only when the model is missing, partial or quarantined. A model on disk that start-up has not hashed yet reads "unverified: on disk, checked before the voice gate first uses it" (QA F1, R3), and the `voice gate` label no longer runs into its value. docs/dictation.md now says silence_dbfs retired silence_peak_threshold (QA F2, R8). QA F3 (stop on a gated take answers CONFLICT) is left to its own child.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran [2026-11-01] codex gpt-6.1-sol at high; NEEDS_WORK on R1 resolved by the human decision and b827fc4, re-review SHIP; QA follow-up 088517b reviewed against e8d5d20, SHIP with no findings

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 8f9aabd65c77b608eedb1727927b5c1295c34a3b, 37af701cb397a96f79cea3ec0d3207260b5be58c, 5f78e079ce5315a8c886f59efc8f58c0258585a4, 5d405bf70b18e5ce58fc252cb3f6bf10f02a5643, 672b334e60896f419794eb4e7581f9b6e28fab5a, 34ed1f90beb48fa7d4ae760dcd4122f48aef904f, b827fc4c414775dfda0870a3a557915e88d4b9d5, 088517bb66674cb1029eaab8f1435c4c87036b81
- Tests: cargo test -p dettivo-audio -p dettivo-session -p dettivo-speech -p dettivo-transcribe, cargo clippy -p dettivo-audio --all-targets (warnings denied); cargo fmt --check -p dettivo-audio, cargo test -p dettivo-cli, just lint
- PRs: