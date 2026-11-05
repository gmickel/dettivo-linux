---
title: Doctor tells a user to download a present silero-vad during start-up verify
date: "2026-11-01"
track: bug
category: ui
module: crates/dettivo-cli/src/doctor_report.rs voice gate row
tags: [qa, fn-79-clean-dictation-takes-a-real-speech, dictation]
problem_type: ui
symptoms: "voice gate row reads 'unverified: ... run dettivo speech download --model silero-vad' until start-up hashing finishes"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10]
---

## Problem
A user who runs `dettivo doctor` just after the daemon starts, with `whisper/silero-vad` on disk but not yet verified (no manifest, start-up hashing still running), is told the model needs downloading and that dictation gates on level alone. A few seconds later the same row says `ready` and the voice gate is in use.

## Steps to reproduce (cold)
1. Isolated profile (QA harness `early_doctor.sh`): models dir with `whisper/tiny.en` (manifest copied) and `whisper/silero-vad/ggml-silero-v5.1.2.bin` hard-linked with no `manifest.json`; `[models] verify_on_start` default (true).
2. Start the branch `target/debug/dettivod`; as soon as its socket exists run `dettivo doctor`.
3. Wait for the daemon log line `models on disk verified`; run `dettivo doctor` again.

## Expected
R3: "Without the VAD model ... `dettivo doctor` names the missing model and its download command." The model is not missing, so the row should not ask for a download (it could say verifying).

## Actual
Early: `voice gate  whisper/silero-vad unverified: dictation gates on level alone; run `dettivo speech download --model silero-vad``.
After verification: `voice gate  whisper/silero-vad ready`. Reproduced twice (early-doctor-1, early-doctor-2).

## Evidence
- .flow/tmp/qa-fn-79-clean-dictation-takes-a-real-speech/runs/early-doctor-1/{doctor-early.txt,doctor-after-verify.txt,daemon.log}
- .flow/tmp/qa-fn-79-clean-dictation-takes-a-real-speech/runs/early-doctor-2/
- First seen in runs/smoke-jfk-tiny/doctor.txt

## Traceability
- R-IDs: [R3]   scenario: S8b   driver_rung: CLI over the isolated daemon socket (Xvfb session)
