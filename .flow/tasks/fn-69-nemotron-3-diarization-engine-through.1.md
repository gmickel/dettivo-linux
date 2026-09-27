---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-69-nemotron-3-diarization-engine-through.1 Implement Nemotron 3 Diarization engine through NeMo-Speech.cpp

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
NVIDIA Nemotron 3 Diarization is a product engine: `crates/nemo-speech-cpp-sys` builds NeMo-Speech.cpp at commit `97a15afa` (after v0.1.0, which predates the model), its ggml and a static SentencePiece from checksummed archives, and `dettivo-engine-nemotron` runs it on the CPU, ggml Vulkan or ggml CUDA behind the existing diarization protocol. `[meetings.diarization] model = "nemotron-3-diarization"` selects it and the catalogue downloads it. It ships opt-in, because the R4 rule fails on German.

- R1: the daemon's pass ran through the CPU, Vulkan and CUDA builds on this machine (`meetings_diarize_nemotron`: engine `vulkan` and `cuda`, reason naming the RTX 4090). `dettivo doctor`'s diarization row follows the configured model set to its engine's backend and reason. `[engines.diarize] backend` gains `vulkan`, and Settings offers it.
- R2: `scripts/qa/nemotron3/parity_engine.py` against the fp32 fn-64 port on the four AMI test meetings, on each backend. Tolerance fixed before the first run, per meeting: turn DER at most 2.0%, confusion at most 0.5%, decision flips at most 1%. Measured: turn DER 0.24-0.90%, confusion at most 0.061%, flips at most 0.087%. All within tolerance (`docs/reports/benchmarks/diarization-nemotron-engine-2026-09-26.md`).
- R3: fn-67 bench under this branch's rule (before fn-68), Nemotron minus current, paired 95% intervals. AMI dev headline −6.54 [−11.00, −2.44]; AMI test −1.00 [−2.65, +1.48]. No German labels exist, so German has proxies only: remote lines unlabelled +2.20 [+1.18, +4.42], local/remote proxy −0.76 [−1.58, −0.40]. AMI test wall times: CPU 226.6 s (33x), Vulkan 31.5 s, CUDA 24.9 s (304x). The current engine takes 557 s (fn-64 receipt).
- R4: ahead on the pooled headline, behind on German unlabelled lines, so it ships opt-in. ADR 0073 records the numbers. The conductor's re-run after fn-68 decides again; flipping the default is `Diarization::default` plus the default TOML text and its two config fixtures.
- R5: over eight expected speakers and a missing Nemotron model both fall back to `diarization-en`, with `diarization.fallback_reason` on the row (`crates/dettivod/src/diarization_choice.rs` unit tests, `crates/dettivod/tests/meetings_diarize_nemotron.rs`). A failed GPU load falls back to the CPU for `auto`, with `fallback_reason` (`crates/dettivo-engine-nemotron/src/backend.rs` tests). More than eight speakers at the engine is `bad_request` (`tests/cli.rs`).
- R6: `docs/engines.md`, `docs/config.md` and `docs/meetings.md` describe the engine, OpenMDW 1.1, the backends and the fallback, and `docs/models.md`, `docs/install.md`, `docs/diarization-bench.md`, NOTICE, CHANGELOG and ADR 0073 (amending 0004, 0035 and 0057) follow.
- Also: frame probabilities for fn-70 (`diarize` with `frame_probabilities` answers `frames` plus one `probs_f32` attachment; CLI `--probs` writes `.npy`). The bench gains the `nemotron-cpp` engine and skips an engine whose binary is not installed. The package ships the Vulkan engine and its two libraries, and the CUDA drop-in carries both diarization engines (eleven files). CI fetches the Nemotron model.

Conditions: the current engine's and Whisper's bench inputs are cached results from the installed 0.2.x build (same source as 0.3.0). Timings ran under load from other workers. A CPU bench run of all 42 recordings was stopped after five, and the CPU build's accuracy rests on parity. The baseline was not run before the first edit.

Follow-ups: German labels (fn-72) would turn R3's German proxies into a headline; the daemon does not request frame probabilities yet (fn-70); NeMo-Speech.cpp is pinned to a `main` commit, so a release tag should replace it when one ships.

Tier: implementer claude-opus-5-5 (project routing block)

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: 33554d12f8f1ad5bb84d738afa82465105c6b6e6
- Tests: just build test lint (suite_rc=0 on 33554d1; green receipt 33554d12), baseline: not run before the first edit (worker omission); the base f584158 is the closed fn-67 tree, and the three failures seen on the way (doctor and contract engine lists, doctor fake daemon) were caused by this change, cargo test -p dettivo-engine-nemotron -p dettivo-engine-proto with DETTIVO_TEST_NEMOTRON_MODEL, cargo test -p dettivod --test meetings_diarize_nemotron on the CPU, Vulkan and CUDA builds (engine loaded vulkan and cuda on the RTX 4090), scripts/qa/nemotron3/parity_engine.py on AMI test, CPU/Vulkan/CUDA, all within tolerance, just diar-bench --heldout (private eval dir, CUDA product engine), scripts/packaging/test-package-cuda.sh, scripts/packaging/test-check-manifest.sh
- PRs: