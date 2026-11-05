---
title: "Whisper dictation user saying 'exam period' gets 'exam..' — daemon path differs "
date: "2026-11-02"
track: bug
category: runtime-errors
module: dettivod dictation path (Whisper)
tags: [qa, fn-89-whisper-vocabulary-without-false, whisper]
problem_type: runtime-error
symptoms: "guard-library pastes 'during the exam..' through the daemon (off, on, and at base); the bench writes 'exam period.'"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/duck-ramp-overwrites-a-volume-the-user-2026-11-01, bug/runtime-errors/parakeet-dictation-user-gets-fewer-2026-11-01, bug/runtime-errors/parakeet-user-at-strong-loses-the-2026-11-02, bug/runtime-errors/whisper-dictation-user-cant-paste-a-2026-11-02, bug/runtime-errors/whisper-vocabulary-check-panicked-on-2026-11-02]
---

## Problem
A Whisper dictation user who says "... during the exam period." through the daemon gets "... during the exam.." pasted: Whisper reads the spoken word "period" as punctuation and the paste ends in a double full stop. The term-recall bench, fed the same clip through the engine CLI, writes "exam period." The difference is in the daemon's dictation path, not the vocabulary: it happens with the vocabulary off and on, and at the pre-fn-89 base. Pre-existing; not caused by fn-89.

## Steps to reproduce (cold)
1. Isolated profile inside scripts/qa/xvfb-session.sh, debug `dettivod`, release Vulkan Whisper large-v3-turbo, `language = "en"`, `DETTIVO_QA_MODE=1 DETTIVO_MOCK_MIC=crates/dettivo-qa/fixtures/vocabulary/guard-library.wav DETTIVO_MOCK_INSERT=1`, vocabulary off.
2. `dettivo dictation start`, wait the clip length + 0.4 s, `dettivo dictation stop`, read inserted.txt.

## Expected
The guard sentence as said: "The library extended its opening hours so that students could study late during the exam period." (set.json); the bench writes this.

## Actual
"The library extended its opening hours so that students could study late during the exam.." at HEAD 9b1f92e (vocabulary off and on) and at base e0071ee (vocabulary on).

## Evidence
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/set-off/guard-library/inserted.txt
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/set-on/guard-library/inserted.txt
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/base/set-on/results.tsv
- bench: .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/bench/evidence/run-1793618001-1642401/bench-vocabulary/vocabulary-report.json

## Traceability
- R-IDs: none (regression note)   scenario: S1   driver_rung: CLI + daemon socket (mock mic)   viewport: n/a
