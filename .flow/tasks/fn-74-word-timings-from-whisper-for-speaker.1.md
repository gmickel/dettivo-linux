---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-74-word-timings-from-whisper-for-speaker.1 Implement Word timings from Whisper for speaker labelling

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Whisper now times every word by DTW (alignment-head preset read from the ggml header, plain token timestamps as the fallback, no words under VAD), and each segment's span follows its words. Meetings, imports and re-runs store the words. A logits filter keeps whisper.cpp 1.8.3's DTW median filter from aborting the engine, which it did on one AMI meeting and on the 2-hour retained meeting before the guard. `pause_ms` defaults to 500, tuned on AMI dev. ADR 0074 records the method, accuracy, cost and outcome.

- R1 met. Every segment carries words (CLI test, long-import test, meeting finalisation through `meeting_asr`). AMI dev words against the manual times: start 170/430 ms, end 40/630 ms, pooled 120/490 ms (median/p95, 61,697 pairs). jfk golden with large-v3-turbo: 102/437 ms pooled.
- R2 missed. Labelled lines with the wrong speaker on AMI dev: 21.02% (current) and 11.24% (Nemotron), against the targets of 15.4% and 5.5% within a point. Blank remote lines on the ten retained meetings: 0.36% and 1.50%, under 3%. ADR 0074 says why: a pause cut needs a gap of `pause_ms` at a diarized change, and the turns place the change a few hundred ms off, so shorter pauses name more lines wrongly and longer ones add nothing over segment boundaries.
- R3 met. AMI dev attribution error 22.43% to 21.07% (current) and 14.65% to 12.49% (Nemotron). Held-out AMI test 16.05% to 14.11% and 14.70% to 11.96%. A no-words control on the same transcripts gives 21.07% and 12.75%, so most of the gain comes from DTW-snapped segment spans, and the snapped lines hold about 5% fewer reference words.
- R4 met. The jfk text is identical with and without words on tiny.en, base.en and large-v3-turbo. import-merge stays at WER 0.000. Added time per meeting hour: about 3 s on the RTX 4090. CPU shows no difference beyond the 10.7 s run-to-run spread. The integrated Radeon adds 1.1 to 6.9 min, measured while it was shared.
- R5 met. ADR 0074, with a forward pointer on ADR 0072. `docs/meetings.md` and `docs/engines.md` say meetings carry word timings, and config, history, API deltas, the bench doc and the changelog are updated.
- Bench (additive): `"asr": {"meetings": true}` transcribes retained meetings through the product finalisation, and `scripts/qa/diarbench/word_timing.py` measures word times on AMI. The alignment pipeline now measures large-v3-turbo. Aggregate report: `docs/reports/benchmarks/diarization-bench-2026-09-27-word-timings.{md,json}`.
- Residual risk: a final audio window of 110 to 150 ms that still decodes text would abort DTW the same way. None of the 32 bench runs hit it. The lasting fix is a guard in whisper.cpp (follow-up).
- Follow-up: the upstream whisper.cpp DTW guard. R2 needs diarization-side work (turn boundaries, fn-70's voice check).

Tier: implementer claude-opus-5-5 (project routing block)

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: d3f648be1e65a36b4bc72920091e5f0b0fdb1178, d9936ddc09b7cc238bfb89d38dbca13680756ec9
- Tests: just build test lint (baseline: green), just build test lint, cargo test -p dettivo-engine-whisper, cargo test -p dettivod --test import_long, cargo test -p dettivo-meeting, python3 scripts/qa/test_diarbench.py, dettivo-qa pipeline import-merge --cpu, dettivo-qa pipeline parakeet-alignment --cpu, python3 scripts/qa/diarbench/bench.py --heldout --report (DETTIVO_EVAL_DIR overlay, asr.meetings), python3 scripts/qa/diarbench/word_timing.py [--heldout]
- PRs: