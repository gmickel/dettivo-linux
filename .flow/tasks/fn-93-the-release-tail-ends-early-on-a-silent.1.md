---
satisfies: [R1, R2, R3, R4]
---
# fn-93-the-release-tail-ends-early-on-a-silent.1 Implement The release tail ends early on a silent ending

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
A take whose 150 ms before the release holds no speech by the gate's rules now ends at the release instead of after the 200 ms tail. The level stage judges the stretch first (loudest 30 ms window under silence_dbfs); when it hears sound, the speech check that ran during the take answers whether any Silero window there reached 0.4 (new optional `threshold` on `feed_speech`, answered by `voiced_until`). The take ends at the first 512-sample window boundary at or after the release, so the check has run every sample kept. Speech in the stretch, no streaming detector, a failed check or a late answer keep the 200 ms tail and TAIL_STALL_GRACE unchanged.

R1/R2: crates/dettivo-session/tests/silent_ending.rs (silent ending under digital silence and a hum ends at the release; speech ending keeps the tail; a check that cannot answer and a whole-take detector keep it; words ending 60/100/140 ms before the release keep every sample and the whole tail), crates/dettivo-engine-whisper/tests/vad_voiced.rs (voiced_until on jfk mid-sentence, in its pause, on silence; asking leaves the verdict unchanged). fn-79's and fn-90's gate tests pass unmodified (gate.rs, speech_check.rs, vad_stream.rs, take_window.rs, stop_boundary.rs).

R3: paired before/after daemons on the RTX 4090 (Vulkan, Parakeet Ultra): silent endings -176 ms paired p50 on jfk (21/21 faster; 354 -> 192 ms p50; capture 220 -> 40 ms) and -197 ms on digital silence (5/5); speech endings +2 ms on jfk (9 faster, 11 slower, sign test p = 0.82) and -9 ms on the level clip, capture 220 ms in every take. Block run 1 agrees (278 -> 95 ms silent, 286 -> 283 ms speech over 60 takes). Typed text still waits for the 250 ms modifier settle (ADR 0090), so short takes type at about 270 ms either way. Report: docs/reports/benchmarks/release-tail-2026-11-06.md and .json.

R4: ADR 0093 amended in place (rule, 150 ms choice from 52 History takes: 46 silent endings at 150 ms, 50 at 100, 38 at 200; measurement), ADR 0097 note, docs/dictation.md, docs/insertion.md, docs/engines.md, CHANGELOG [Unreleased].

stage: impl-review - ran [round 1 NEEDS_WORK (window anchored to the cut instead of the release; typed-text overclaim), both fixed; round 2 SHIP] (model: gpt-6.1-sol)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: b53d516bd475c7cb450f5fe39aed451b00b0cdad, 8b27e8bfac7727df6249aeafa73de085660699e3
- Tests: cargo test --no-fail-fast -p dettivod -p dettivo-session -p dettivo-speech -p dettivo-engine-proto -p dettivo-engine-whisper, cargo test -p dettivo-session --test silent_ending, just lint, R3 paired daemon bench ~/.cache/agent-work/fn93 (pair.sh)
- PRs: