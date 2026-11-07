---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-92-dictation-decodes-while-the-key-is-held.1 Implement Dictation decodes while the key is held

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Parakeet dictations on a GPU are decoded in rounds while the key is held (1.5 s cadence, 20 s context and horizon, commits from 60 s); the release decodes a ~4 s tail plus 2 s overlap stitched by word centres, or reuses the last round when the speech check heard no voice after it, and starts the tail decode once the take's samples are final. Whisper, CPU and short takes keep the whole-take decode. History records timings.release_decode. `dettivo-qa bench windowed` scores the WER cost. ADR 0102 amends 0094; docs/dictation.md, docs/engines.md, docs/qa.md, CHANGELOG and docs/reports/benchmarks/dictation-rounds-2026-11-07.{md,json} follow.

R1 (shipped build, 5 takes each, Ultra/Vulkan, RTX 4090): p50 88/114/273 ms, slowest 90/115/283 ms (before: 349/525/841). R2 (4 s window, pooled 30/60/120 s FLEURS takes, 95% paired bootstrap): Ultra en -0.0019 [-0.0035,-0.0005], Ultra de -0.0016 [-0.0028,-0.0004], v3 en -0.0871 [-0.1079,-0.0681], v3 de -0.0212 [-0.0330,-0.0104]; per-length v3 de 30 s upper bound +0.0068. R5: Whisper en +0.0349 [-0.0037,+0.0755], not shipped.

stage: impl-review - SHIP (codex gpt-6.1-sol high, round 2; follow-up P2: a take that kept its tail without voice reaching the release still starts the early tail decode though reuse may win)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: b757426b90b97e4f34c28800d03e2997b1f87ac6, 42de8ceddd89377801630dd65daff35b8e8852eb, 63649635c73e57223d94160cc51056f99bf084db, 6da7adc013c19f80b6edb69b7405567ee5883f77
- Tests: cargo test -p dettivo-session -p dettivo-transcribe -p dettivo-engine-parakeet -p dettivod -p dettivo-storage -p dettivo-qa
- PRs: