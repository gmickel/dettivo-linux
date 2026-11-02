---
title: Whisper dictation user saying a stop beside the engine's own mark gets two marks
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-language/src/raw/mod.rs (punctuate)
tags: [qa, fn-89-whisper-vocabulary-without-false, raw-punctuation]
problem_type: runtime-error
symptoms: "'Is it done question mark' pastes 'Is it done??'; 'report period, then' pastes 'report., then'"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
A Whisper dictation user who says a punctuation command where Whisper already wrote a mark next to it gets two marks pasted. The "ends the sentence once" rule from 2b527e6 only drops a second `.`. It does not drop a second `?` or `!`, and it does not drop a comma that comes after the stop.

## Steps to reproduce (cold)
1. Isolated release dettivod at 0cb45a8 (mock mic, mock insert), Whisper large-v3-turbo over Vulkan, `[dictation] mode = "raw"`, `spoken_punctuation = true` (defaults), five-term vocabulary. Harness: .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/r3/harness.sh.
2. Dictate the clip r3/extra/punct-question-amy.wav (Piper amy: "Is it done question mark").
3. Dictate r3/extra/punct-report-stop-amy.wav (Piper amy: "Send me the report period. Then go home.").
4. Read the pasted text (state/dettivo/qa/inserted.txt).

## Expected
docs/dictation.md (raw mode): "A spoken stop beside the stop the engine wrote ends the sentence once". The unit test case is `("is it done question mark.", "Is it done?")`.

## Actual
- punct-question-amy pastes `Is it done??`. With spoken punctuation off, Whisper wrote `Is it done? Question mark.`, so the spoken `?` lands after the engine's `?`.
- punct-report-stop-amy pastes `Send me the report., then go home.`. Whisper wrote `Send me the report, period, then go home.`. The comma before the stop gives way, but the one after it stays, and `then` keeps its lower case.
- Both came out the same on two runs (r3/w-punct-on and r3/w-punct-on-2). The ryan rendering of the report clip pasted `Send me the report. And go home.`, which is correct.
- Not caused by the vocabulary check: candidates=0 and no second decode on every take. The raw pass (`punctuate` in crates/dettivo-language/src/raw/mod.rs) treats a doubled mark as a duplicate only when the second mark is `.`.

## Evidence
- results: .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/r3/w-punct-on/results.tsv, r3/w-punct-on-2/results.tsv
- engine text: .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/r3/w-punct-nopunct/results.tsv
- daemon logs: r3/w-punct-on/punct-question-amy/daemon.log, r3/w-punct-on/punct-report-stop-amy/daemon.log
- url: n/a (daemon IPC; dictation.start/stop over a private socket)

## Traceability
- R-IDs: [] (regression scenario on the QA-fn89-3 fix; fn-89 spec has no punctuation criterion)   scenario: S8   driver_rung: daemon IPC + mock insert   viewport: n/a
