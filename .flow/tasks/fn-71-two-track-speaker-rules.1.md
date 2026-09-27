---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-71-two-track-speaker-rules.1 Implement Two-track speaker rules

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The speaker pass of a two-track meeting now applies the fn-71 rules around the sentence rule (`crates/dettivo-meeting/src/two_track.rs`, ADR 0075). A level bleed gate cuts the time "You" lines spend on remote-only speech from 37.86% to 22.00% on the ten retained meetings, at a cost of 0.32 points of local speech. One engine speaker labels every remote line, a silent system track switches to the shared microphone, and a local voiceprint names remote lines in the user's voice You.

Rule by rule on the bench, measured against every rule off (which reproduced the pre-change baseline exactly):
- R1 bleed gate (`bleed_min_voiced` 0.05): You on remote-only speech -15.86 [-21.83, -9.30]; local speech outside You +0.32 [+0.21, +0.41]; 15.2% of microphone lines dropped. The sweep: 0.02 gives -10.72 for +0.06, and 0.10 gives -21.73 for +1.09.
- Text gate (trigram echo within 6 s): -0.00 leak, +0.07 cost, 0.11% of lines dropped. It fails R5 and is not shipped. The cheap gate met R1, so no echo cancellation.
- R2 single remote: fires on 2 of 10 meetings; Nemotron's blank remote lines go from 2.40% to 2.23%. It rests on the engine alone. Deviation from the spec: the spec wanted the engine and a voice check to agree, but embedding voice checks could not tell one voice from two. One voice's microphone lines split as far apart (0.05 to 0.77) as several remote voices (0.04 to 0.52), and the cluster-centroid merge never fired. Both were measured and dropped (see the ADR).
- R3 shared mic: 0 of 10 real meetings switched, which is the false-switch check on real data. Covered by `a_silent_system_track_switches_to_the_shared_microphone` (fixture meetings built from `two-speakers.wav`, with a normal two-track meeting as the negative case).
- R4 voiceprint: `<data_dir>/voiceprint.json` (mode 600, opt-out `voiceprint = false`, keyed to the model set, idempotent per meeting). It uses a new `embed` request on `dettivo-engine-diarize` (ERes2Net, 192 dimensions). 0 remote lines relabelled on the bench: the user's lines sit at a median of 0.73 to their own voiceprint, and no remote line exceeds 0.47. `voice_match` is 0.6.
- R5: AMI dev headline +0.00 with both engines (22.43% / 14.65%); AMI test 16.05% / 14.70%. The rules skip room audio. The AMI Whisper cache was stale (pre word-timings) and not recomputed, so the absolute AMI figures differ from ADR 0074's while the deltas stay zero. The local meetings have no labels, so they have no headline; the proxies stand in.
- R6: `docs/meetings.md`, `docs/config.md`, `docs/diarization-bench.md`, `docs/engines.md`, ADR 0075 (0035 and 0072 cross-linked), CHANGELOG, and the aggregate-only report `docs/reports/benchmarks/diarization-bench-2026-09-27-two-track.{md,json}`.

Tests: `two_track_tests.rs` (bleed gate, single remote, shared mic on a fixture plus the no-switch case, voiceprint relabel), plus `levels.rs`, `voiceprint.rs`, the embed host round trip in engine-proto, a real-model embed test in engine-diarize, and the config validation cases.

Baseline: green (`just build test lint` pre-edit). Gate: `just build test lint` green on 6cf3fb2.

Follow-ups, not built: naming the user in room audio from the voiceprint; a labelled local set so the local meetings get a headline; recomputing the AMI Whisper cache with `--full`. The settings route gains five rows (Meetings / Speakers); the user's route to them is otherwise unchanged.

stage: impl-review - skipped(config: REVIEW_MODE=none)

Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: 6cf3fb27277aaed88ec2db22c6fbee3573fd08d4
- Tests: just build test lint, just diar-bench --save fn71-before, just diar-bench (rules off, per-rule and bleed_min_voiced sweep vs fn71-off), just diar-bench --heldout --report
- PRs: