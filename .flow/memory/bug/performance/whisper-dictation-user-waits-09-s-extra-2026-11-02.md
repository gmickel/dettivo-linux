---
title: Whisper dictation user waits ~0.9 s extra on a lone vocabulary name — term check
date: "2026-11-02"
track: bug
category: performance
module: crates/dettivo-engine-whisper/src/check.rs (prompt-free decode)
tags: [qa, fn-89-whisper-vocabulary-without-false, whisper, vocabulary]
problem_type: performance
symptoms: "check_ms 804-1210 on single-name Omarchy/SapienXT takes, reproduced twice; bench clips 80-122 ms"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
A Whisper dictation user who says a listed name on its own waits about a second longer for the paste on some takes: the term check's prompt-free second decode of a short take ("Oh marky.", "Sapien X T.") took 804 to 1210 ms, against the spec's 150 ms target for clips with a candidate. On the 47 benchmark clips the three second decodes took 80 to 122 ms, so the R4 median holds; the slow cases are short single-name takes outside the benchmark set.

## Steps to reproduce (cold)
1. Isolated profile inside scripts/qa/xvfb-session.sh, debug `dettivod` at 9b1f92e, release Vulkan `dettivo-engine-whisper` at 9b1f92e (RTX 4090), Whisper large-v3-turbo, `language = "en"`, the fixture set's five-term vocabulary, `DETTIVO_QA_MODE=1 DETTIVO_MOCK_MIC=<clip> DETTIVO_MOCK_INSERT=1`; wait for `engine warmed up` before the take.
2. Dictate single-omarchy-amy, single-omarchy-ryan, single-sapienxt-amy (from .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/single/).
3. Read the engine's `vocabulary check ... check_ms=` line.

## Expected
Spec R4: "Added latency is reported per clip; the median on clips with a candidate is under 150 ms on the reference GPU." Approach: "the target is a median under 150 ms extra on clips with a candidate".

## Actual
check_ms run1 / run2: single-omarchy-amy 854.2 / 896.2, single-omarchy-ryan 804.3 / 895.7, single-sapienxt-amy 1209.7 / 993.2. Other single-name second decodes: 84 to 137 ms. Over all 10 single-name takes the median is about 100 ms, so it is the short echo-shaped "Omarchy"/"SapienXT" takes that cost about 0.9 s (the prompt-free decode of a padded 1.5 s take is probably falling back through temperatures; unconfirmed). GPU shared with other agents (2 to 64 % busy at sample points); reproduced on the rerun.

## Evidence
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/single-on/results.tsv, run2/single-on/results.tsv
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/single-on/single-omarchy-amy/engine.log (and siblings)
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/gpu.txt, run2-gpu.txt

## Traceability
- R-IDs: [R4]   scenario: S4-latency   driver_rung: CLI + daemon socket (mock mic)   viewport: n/a
