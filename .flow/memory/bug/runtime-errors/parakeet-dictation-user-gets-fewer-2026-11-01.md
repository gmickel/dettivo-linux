---
title: Parakeet dictation user gets fewer vocabulary terms than the benchmark reports —
date: "2026-11-01"
track: bug
category: runtime-errors
module: crates/dettivo-qa/src/bench/vocabulary_run.rs / dictation take padding
tags: [qa, fn-84-vocabulary-on-parakeet, parakeet, vocabulary]
problem_type: runtime-error
symptoms: Daemon dictation at normal recovers 7 of 12 missed terms (10/15) vs the bench's 8 of 12 (11/15); target-hyprland-1 stays Hyperland
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/vocabulary-rescoring-rewrote-part-of-a-2026-11-01]
---

## Problem
A Parakeet user dictating a vocabulary term through the real dictation path at the default `normal` boost gets fewer terms back than the term-recall benchmark reports. The bench feeds each clip to the engine as is; a dictation take reaches the engine with 400 ms of trailing silence (docs/dictation.md), and that pad moves one knife-edge clip from a pass to a miss. Through the daemon, `normal` recovers 7 of the 12 missed terms, under the two-thirds target the benchmark shows it meeting (8 of 12).

## Steps to reproduce (cold)
1. Isolated profile, debug `dettivod` at 87d6a52 inside scripts/qa/xvfb-session.sh, `DETTIVO_QA_MODE=1 DETTIVO_MOCK_MIC=<clip> DETTIVO_MOCK_INSERT=1`, `[engines] directory` = the release Vulkan engines, `[speech] provider = "parakeet"`, `model = "parakeet-v3"`, `[dictation] language = "en"`, the fixture set's five-term vocabulary, `vocabulary_boost = "normal"`.
2. For each `crates/dettivo-qa/fixtures/vocabulary/target-*.wav`: `dettivo dictation start`, wait the clip length, `dettivo dictation stop`, read `inserted.txt`.
3. Compare with `dettivo-qa bench vocabulary` at the same head (11/15 at normal).

## Expected
R2: "On the term-recall benchmark at `normal`, term recall rises clearly over `off` (target: at least two thirds of missed terms recovered)." The benchmark meets it (11/15, 8 of 12). The docs say `normal` "brings back 8 of the 12 terms Parakeet misses".

## Actual
Daemon path at normal: 10/15. `target-hyprland-1` stays "Hyperland draws the windows ..." in two separate runs (run1, run2), while the bench writes Hyprland. Engine CLI on the same audio: fixture as is margin +0.10 nats (replaced); the stored take (`microphone.wav`) plus 400 ms of silence, as the session sends it, margin -0.76 nats (not replaced). At `strong` the daemon path writes Hyprland. No false insertion was seen on the daemon path.

## Evidence
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/s2-daemon-targets.log (15 targets through the daemon at normal)
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/s2-margin/*.log (margins: fixture +0.099, stored take +0.106, take + 400 ms pad -0.762)
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/run1/s1-parakeet/target-hyprland-1/, run2/s1-parakeet/target-hyprland-1/
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/s3-bench/bench-run1.log

## Traceability
- R-IDs: [R2]   scenario: S2b   driver_rung: CLI + daemon socket (mock mic)   viewport: n/a
