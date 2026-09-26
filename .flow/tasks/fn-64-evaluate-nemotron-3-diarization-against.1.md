---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-64-evaluate-nemotron-3-diarization-against.1 Implement Evaluate Nemotron 3 Diarization against current speaker pass

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Measured NVIDIA Nemotron 3 Diarization (a Python/ONNX port of omarchy-meeting-recorder v1.1.0's pass, cross-checked against the transformers reference) against the shipping `dettivo-engine-diarize` on CPU and CUDA. The report is docs/reports/benchmarks/diarization-nemotron3-2026-09-25.{md,json}, the scripts are under scripts/qa/nemotron3/, and R5 was appended to the spec.

- R1/R2: on the AMI test split (4 meetings, not in Nemotron's training data), the wrong-speaker rate (DER confusion, ADR 0058 scorer) is 3.39% for the baseline against 0.71% for Nemotron. Missed speech is 9.25% against 20.16%, and total DER is 15.67% against 21.78%. CPU wall time is 557 s against 138 s, a quarter. CUDA takes 510 s against 14 s. A known-count control and train-split continuity rows are included.
- Real meetings: 8 retained meetings, 4 EN (277 min) and 4 DE (139 min, 3 from 2026-09-25). No human labels exist. The proxies are local/remote confusion against a mic/system track-energy reference on the mix (pooled 0.88% to 0.45%, EN 0.63% to 0.57%, DE 1.37% to 0.21%), overlap frames with both sides active (75.0% to 78.3%), and a blinded model judge on the product's system-track transcripts (pooled 0.28% against 0.57% wrong lines, below the judge's resolution, with the systems agreeing on 683 of 688 lines).
- R3: rates, times, the CPU/CUDA runtime identities and hashes, and the overlap result are recorded in the report and its JSON. R4: no daemon, package, config or default change. R5: German is reported separately from English and pooled.
- Open item: speaker-turn labels from a person on 2 or 3 multi-speaker retained meetings (at least one German) would turn the proxies into a true wrong-speaker rate. Confidential working files stay in ~/.local/share/dettivo-eval/fn64 (mode 700), outside git.
- Follow-up candidates, not built: Nemotron speaker labels over the current engine's speech regions, which would avoid Nemotron's missed speech. The baseline's CPU and CUDA builds diverge in known-count mode on IS1009b (17.96% against 3.21% confusion).

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: 50d458d6d3f2751f3c825ed29ed6f3e94ff7f2bc
- Tests: just build test lint (baseline: green pre-edit), just build test lint (post-commit, suite_rc=0), python3 scripts/qa/nemotron3/crosscheck_reference.py (fp32 port vs transformers: max prob diff 1.1e-5, 0 decision flips), python3 scripts/qa/nemotron3/score_eval.py over 104 recorded runs
- PRs: