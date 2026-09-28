---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-70-voice-check-for-each-transcript-unit.1 Implement Voice check for each transcript unit

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Each sentence unit is now checked against the speakers' voices inside the sentence rule (`crates/dettivo-meeting/src/voice_check.rs`, ADR 0076): centroids from confident units, a move at agree 0.15 when the unit's turns also name the speaker or overrule 0.2 on the voice alone, both scaled per recording; `[meetings.diarization] voice_check` (default true) switches it off. On the fn-67 bench (check off vs on, ADR 0074 inputs reproduced exactly) AMI dev attribution error falls 21.07% -> 18.60% (current) and 12.49% -> 11.98% (Nemotron), held-out AMI test 14.11% -> 11.85% and 11.96% -> 11.52%; words fixed/broken 3.90%/1.41% and 1.57%/1.05% on dev (re-measured after the PR #26 review fix that keeps units no turn overlaps out of the confident set; before it 18.61%, 11.98%, 11.81%, 11.54%). Short-turn error falls on dev for both; on test it falls for the current engine and its Nemotron interval spans zero. Local meetings have no labels, so no headline there; German moves 0.84%/0.37% of units with every proxy unchanged. Nemotron frame probabilities as evidence gave -0.03 [-0.30, +0.32] and are not used. Cost: ~24 s per meeting hour on CPU, 33 s on CUDA (release build); no written pass budget, so the ADR states it. Merged-cluster probe: AMI unit voices separate two merged speakers (AUC 0.94), recorded as a follow-up.

Tests: voice_check_tests.rs covers the agree branch, the voice-only branch, the per-recording scale and floor, centroids from confident units, the 500 ms floor and a moved sentence through diarize::assign (R6); test_diarbench.py covers the fixed/broken metric.

Baseline: red pre-edit (meetings_diarize_crash: the installed Vulkan whisper engine crashed while a game held the GPU; inherited, passed in the final gate). Final gate: `just build test lint` run 1 hit a load-timing flake in config_live (passes alone twice); run 2 had build and every test green and failed only test-check-docs because the new ADR was untracked; after the commit `just lint` is green.

Follow-ups: a merged-cluster detector from unit voices; German labels (fn-72) to confirm R3 with a headline; the scratch bench driver used to compute embeddings without rerunning stale engine/Whisper results is not in git.

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: 35b3c4cc68f04f47dc6b4d9f3023c9cc7d361b2a
- Tests: MISE_JUST_VERSION=1.58.0 just build test lint (build and all tests green; docs self-check failed only on the then-untracked ADR), MISE_JUST_VERSION=1.58.0 just lint (after commit, green), cargo test -p dettivo-meeting --lib, python3 scripts/qa/test_diarbench.py, just diar-bench --heldout --baseline fn70-off-heldout --report, baseline: red (meetings_diarize_crash: installed Vulkan whisper crashed while the GPU was busy; inherited)
- Re-measure after the review fix (commit e209d5f tree, cached embeddings re-keyed, no engine/ASR/embedding recompute): just diar-bench --heldout with voice_check false and on; check-off reproduced fn70-off-heldout exactly
- PRs: