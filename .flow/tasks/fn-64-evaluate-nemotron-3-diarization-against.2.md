---
satisfies: [R6]
---
# fn-64-evaluate-nemotron-3-diarization-against.2 Score Nemotron with segment-based speaker assignment

## Description
TBD

## Acceptance
R6 in the parent spec is satisfied; judge against it directly.

## Done summary
Scored Nemotron with a segment-based speaker adapter (scripts/qa/nemotron3/segment_assign.py). The adapter gives each transcript line the kept-channel speaker with the highest mean probability, with no 0.5 threshold. It is compared with the current engine and with Nemotron under the recorder post-processing, both going through the speaker pass's own rule (ported to Python; it reproduces the stored product labels on 4,534 of 4,545 remote lines). On the Dettivo meetings the rule leaves 22.7% (current engine) and 24.5% (Nemotron) of remote lines without a speaker, and the adapter leaves 0%. A fresh blind three-column judge pack (seed 65) found 0 wrong lines in every column: English 0/396, 0/396 and 0/475, German 0/303, 0/292 and 0/365. On the AMI test split with Whisper segments, the adapter reaches 3.39% confusion and 16.65% missed speech, against 1.97% and 25.60% for the current engine and 1.78% and 25.95% for the recorder. It gets 86.2% of lines right against 75.6%, and 13.8% wrong against 5.5%. A reference-word segmentation is also reported. The report is updated in place, with the new section, the summary and the conclusions revised and the licence caveat dropped. There are no product code changes (R4).

Tier: implementer claude-opus-5-5 (project routing block)
stage: impl-review - skipped(config: REVIEW_MODE=none)
Follow-up idea (not built): run the adapter only on the lines the rule leaves blank.
## Evidence
- Commits: 400480cc58fc664602f4d6c5748826c6ccfd91a6
- Tests: just build test lint (baseline green pre-edit; verify green, suite_rc=0), score_segments.py port check: product rule reproduces stored labels on 4,534 of 4,545 remote lines, judge_pack.py --rule overlap reproduces the first pack byte-identically; score_eval.judged reproduces committed first-pack aggregates
- PRs: