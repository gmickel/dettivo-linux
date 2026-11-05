---
title: "Dictation user with stt_idle_seconds under 60 gets no Parakeet warm-up after an "
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-speech/src/supervisor_warm.rs
tags: [qa, fn-90-the-speech-check-runs-during-the-take, engines]
problem_type: runtime-error
symptoms: "With stt_idle_seconds = 5, two reloads after idle unloads logged no 'engine warmed up'; the warm event came only after the first take"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/dictation-user-cant-record-after-a-2026-11-02, bug/runtime-errors/doctor-says-microphone-ready-open-after-2026-11-02, bug/runtime-errors/duck-ramp-overwrites-a-volume-the-user-2026-11-01, bug/runtime-errors/parakeet-dictation-user-gets-fewer-2026-11-01, bug/runtime-errors/parakeet-user-at-strong-loses-the-2026-11-02]
---

## Problem
A user who sets `[engines] stt_idle_seconds` below 60 gets no Parakeet warm-up when the engine reloads after an idle unload. The reload's warm-up is skipped because the supervisor's last-recognition time survives the unload, so the warm-up skip (`WARM_UP_SKIP`, 60 s since the last recognition) fires for a freshly spawned engine process.

## Steps to reproduce (cold)
1. Isolated daemon under `scripts/qa/xvfb-session.sh` with Parakeet Ultra on Vulkan, mode `raw`, `stt_idle_seconds = 5`, mock mic (`DETTIVO_MOCK_MIC`) and mock insert (`.flow/tmp/qa-fn-90-the-speech-check-runs-during-the-take/run.sh`, plan `3x2,unload,3x2,unload,3x2`).
2. Dictate two 3 s takes. The first load logs `engine warmed up ... warm_up_ms=229`.
3. Wait until the idle reaper unloads Parakeet (about 18-20 s).
4. Dictate a 3 s take. The press respawns and reloads the engine.
5. Observe the daemon log and the `engine.state` events for that reload. Repeat steps 3-5 once more.

## Expected
ADR 0084/0098: after each Parakeet load on a GPU backend, the warm-up runs, so that no take compiles its own GPU kernels. The skip exists for an engine that "recognised within the last 60 s". A new process has recognised nothing yet.

## Actual
Neither reload logged `engine warmed up`. The `warm` event came at 3,274-3,276 ms after the start, after the take had been recognised, not from a warm-up. On a current shader cache the take after the reload was still fast (274-276 ms release-to-text), so a user sees nothing until the cache is stale (after a driver or engine update). Then a take longer than 40 s after such a reload would compile its own kernels: the 2.5-2.9 s stall ADR 0098 removes. The default `stt_idle_seconds` of 300 cannot trigger it.

## Evidence
- console: .flow/tmp/qa-fn-90-the-speech-check-runs-during-the-take/runs/fu-idle5/daemon.log (one `engine warmed up` line, for the first load only)
- takes: .flow/tmp/qa-fn-90-the-speech-check-runs-during-the-take/runs/fu-idle5/takes.jsonl (engine events per take)
- contrast: runs/r4-idle61/daemon.log, where `stt_idle_seconds = 61` logs a warm-up on all six loads

## Traceability
- R-IDs: [R4]   scenario: FU-idle5   driver_rung: daemon IPC (frame/JSON-RPC over the isolated socket)   viewport: n/a (headless daemon)
- Predates fn-90: `last_recognized` and `WARM_UP_SKIP` come from ADR 0084 (crates/dettivo-speech/src/supervisor.rs, supervisor_warm.rs).
