---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-68-label-every-remote-line-with-sentence.1 Implement Label every remote line with sentence-unit speaker voting

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The speaker pass now labels every remote line with the speaker holding most of its sentence (crates/dettivo-meeting/src/sentences.rs, ADR 0072 amending ADR 0035). Segments are cut at sentence ends and at pauses between aligned words where the speaker changes, a sentence votes once across segments, a line outside every turn takes the nearest turn within nearest_turn_ms (10 s), and a segment whose sentences took two speakers is split into stored segments that the daemon polishes again.

Bench (fn-67, AMI dev, old rule to new): attribution error 28.99% to 22.43% (current engine) and 22.43% to 14.65% (Nemotron); held-out AMI test 22.98% to 16.05% and 21.92% to 14.70%. Remote lines left blank on the 10 retained meetings, pooled: 18.01% to 1.14% and 19.52% to 2.40% (R1, R3 met).

Deliberate misses:
- R2 not met. The share of labelled lines with the wrong speaker rises 5.15 points (current) and 5.57 (Nemotron) on AMI dev, against a bound of 1. The lines the old rule left blank are mostly two-speaker Whisper segments. Without word times no single label gets more than about 52% (current) and 66% (Nemotron) of them right. min_speaker_share = 0.6 (kept, now per sentence) is the lever. It holds the bound for the current engine only and gives back most of the gain. Needs Gordon's call.
- Fragment merging (anarlog-style) was measured and dropped: it raised the headline at every setting tried. R4's fragment test pins that a one-word sentence keeps its own speaker.

Config: pause_ms and nearest_turn_ms added; min_speaker_share now applies per sentence (default 0); min_coverage retired (read and ignored so old files still load). Docs: docs/config.md (migration), docs/meetings.md, docs/diarization-bench.md; new bench metric "Labelled lines, wrong speaker"; report docs/reports/benchmarks/diarization-bench-2026-09-26-sentence-units.md.

Tests: crates/dettivo-meeting/src/sentences_tests.rs (sentence ends, pauses, speaker change across a pause, nearest-turn tolerance, fragment, share floor, room audio, microphone-only, re-run without data loss for R6); daemon re-run assertions in crates/dettivod/tests/meetings_diarize.rs.

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: b682f936d0dba35b7a89bd86ff60ee8e9a9327d4
- Tests: baseline: green (just build test lint, pre-edit), MISE_JUST_VERSION=1.58.0 just build test lint (build, test, lint green through clippy/settings/qt; docs self-check failed only on then-untracked new files), MISE_JUST_VERSION=1.58.0 just docs (green after commit), cargo test -p dettivo-meeting --lib, cargo test -p dettivod --test meetings_diarize --test config_live, python3 scripts/qa/test_diarbench.py, just diar-bench --save fn68-before; just diar-bench --heldout --save fn68-before-heldout (baselines of the old rule), just diar-bench --heldout --baseline fn68-before-heldout --report
- PRs: