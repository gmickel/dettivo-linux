---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-86-streaming-parakeet-dictation-with-live.1 Implement Streaming Parakeet dictation with live text

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
R1 met by ADR 0094, now Accepted 2026-11-02 (Gordon Mickel). It records release-to-text and transcribe_ms against take length per engine and backend: on the RTX 4090 with Vulkan, the 60 s p50 is 683 ms and the 120 s p50 is 1,157 ms, and parakeet.cpp 0.5.0 cannot stream the TDT dictation models. R2-R7 do not apply: the gate decision (Gordon, 2026-11-02, goal fn-88, option B "Close + VAD during take") closed the spec without building streaming. The 120 s miss and the first-take-after-idle stalls go to fn-90-the-speech-check-runs-during-the-take.

Decisions:
- No CHANGELOG line: [Unreleased] records only user-visible changes, and this outcome changes no behaviour.
- docs/engines.md unchanged: it does not describe Parakeet dictation streaming.
- ADR index row status set to "Accepted 2026-11-02; streaming not built", matching the other rows' shape.

stage: impl-review - ran (codex gpt-6.1-sol high: earlier rounds NEEDS_WORK -> SHIP at efa580a; acceptance round SHIP at 3af97e8, no findings)
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 6917dcf6906de1d4552acf0773ff9eeb91be79db, c7141124593a0211881df1fcfb83f7043855179a, efa580ad9d118b9c4d8627e44cd71b4ef339fbcc, 3af97e8e9eda0d3a556e5a538b3865ed2563546a
- Tests: scripts/check-docs.sh (docs: ok, rc=0), just lint (rc=0), GATE_SKIPPED:cargo-test:docs-only - cumulative diff classified tier-B (no executable paths touched)
- PRs: