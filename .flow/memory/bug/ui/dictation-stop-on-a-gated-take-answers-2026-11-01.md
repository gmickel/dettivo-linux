---
title: "dictation stop on a gated take answers CONFLICT sessionCancelled, not silence"
date: "2026-11-01"
track: bug
category: ui
module: crates/dettivod/src/actions.rs dictation.stop
tags: [qa, fn-79-clean-dictation-takes-a-real-speech, dictation]
problem_type: ui
symptoms: stop exits 1 with 'the session was cancelled (CONFLICT)'; the silence reason only reaches the event stream
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10]
---

## Problem
A user who binds `dettivo dictation stop` (or runs it in a terminal) and releases on a silent take gets exit status 1 and `dettivo: the session was cancelled (CONFLICT)`, with `details.kind = sessionCancelled` and no mention of silence. The event stream and the pill say `silence: level` / "Nothing heard", but the stop reply does not.

## Steps to reproduce (cold)
1. Isolated profile, Whisper tiny.en, Silero present, `DETTIVO_MOCK_MIC=reg_stop_before_speech.wav` (1.6 s of room floor, then "Hello world").
2. `dettivo dictation start --language en`; after 1.0 s `dettivo dictation stop --json`.

## Expected
Spec "Reporting": a gated take ends `cancelled` with `reason = "silence"` and the stage. The stop caller could be told that reason.

## Actual
`{"error":{"code":-32013,"data":{"app_code":"CONFLICT","details":{"kind":"sessionCancelled"},"retryable":false},"message":"the session was cancelled"}}`. The `dictation.state` event correctly carries `reason: "silence: level"`. Reproduced twice. The `sessionCancelled` mapping predates fn-79 (`crates/dettivod/src/actions.rs` at the merge base), so this is pre-existing.

## Evidence
- runs/reg1-stop-before-speech/{stop.json,events.jsonl,daemon.log}, runs/reg2-stop-before-speech/

## Traceability
- R-IDs: [R1]   scenario: REG1   driver_rung: CLI over the isolated daemon socket
