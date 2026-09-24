# CUDA diarization missed the 4x speed target on RTX 4090

CUDA diarization is not shipping from this experiment. It preserved CPU speaker turns and produced GPU-process evidence, but failed the required 4x CPU throughput. At the default thread setting, CUDA was slower on all three short samples and on the 300-second repeated-audio check. The canonical benchmark returned `fail` despite matching CPU/CUDA DER of 0.020396270396270396 with two speakers.

## Measured results

Wall time covers a fresh engine process, including model load and inference. Speedup is CPU wall time divided by CUDA wall time; 4.000x is the acceptance target. Realtime factor means audio seconds divided by wall seconds. Three-repeat rows use each provider's median wall time; the diagnostic rows have one run per provider.

| Sample | Audio seconds | Runs per provider | Threads | CPU seconds | CUDA seconds | CUDA speedup |
|---|---:|---:|---|---:|---:|---:|
| `gold` | 18.856 | 3 | default | 1.223 | 2.619 | 0.467x |
| `public-en` | 54.805 | 3 | default | 4.735 | 7.182 | 0.659x |
| `public-zh` | 56.861 | 3 | default | 2.936 | 6.676 | 0.440x |
| `gold`, thread-count diagnostic | 18.856 | 1 | 1 | 2.790 | 2.342 | 1.192x |
| `public-en-loop-300s` | 300.000 | 1 | default | 27.139 | 37.198 | 0.730x |

The single-thread check improved CUDA's relative figure to 1.192x, still below 4x. The 300-second loop measured CPU at 11.054x realtime and CUDA at 8.065x realtime. Lengthening this repeated sample did not produce the required speedup.

The separate canonical `dettivo-qa pipeline diarization --bench` run measured CPU at 1.283 seconds (14.696x realtime) and CUDA at 2.665 seconds (7.075x realtime), for 0.481x speedup. Both found two speakers and the same DER. Its verdict was `fail`, with the speed deficit named in `verdict_reason`.

## What the evidence establishes

All 22 independent comparison invocations exited successfully and reported the requested provider. Each comparison's speaker-turn arrays matched exactly between CPU and CUDA, including timestamps and labels. All 11 CUDA invocations had samples naming their exact engine PID in `nvidia-smi` compute-process output. These checks establish provider parity on these inputs and GPU-process presence; neither is evidence of a speed win.

Only `gold` has the checked-in reference turns used for the DER claim. CPU/CUDA parity on public samples does not establish ground-truth diarization accuracy.

## Meetings-pack integration evidence

The affected `diarization_throughput` step passed on CUDA with engine PID 186509, processing 300,000 ms of audio in 31,214 ms (9.61x absolute realtime). `gpu_workload_proof` passed and found that exact diarization PID in 55 of 57 samples. This meets the pack's absolute 4x realtime floor. It does not meet or replace R2's separate requirement for 4x throughput relative to CPU; that criterion remains failed by the comparisons above.

The overall pack returned exit 1 and `passed = false`. Required GUI steps skipped because `DISPLAY` was unset; preflight also reported an inaccessible accessibility bus. Separately, STT measured 19.78x realtime against its 20x target, recorded as unmet. These results are not a full meetings-pack pass or installed-session approval.

The raw receipt is `/home/gordon/work/dettivo-linux-wt/_factory/fn42-meetings/run-1788936938-175688/pack-meetings/meetings-pack.json`. Its sibling `diarization_throughput/diarization-throughput.json` records the backend, PID and timing; `diarization_throughput/gpu-samples.json` retains process samples. The companion JSON preserves the pack steps, blockers, measurements and diarization throughput payload.

## Inputs and environment

- `gold` is the repository's two-Piper-voice fixture, `crates/dettivo-qa/fixtures/diarization/two-speakers.wav`, with six turns and an overlap. Its reference is `two-speakers.turns.json` beside it.
- `public-en` is the official sherpa-onnx [3-two-speakers-en.wav](https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/3-two-speakers-en.wav) sample, saved locally as `two-speakers-en.wav`; `public-zh` is [0-four-speakers-zh.wav](https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/0-four-speakers-zh.wav), saved as `four-speakers-zh.wav`. The requested speaker counts were two and four.
- `public-en-loop-300s` repeats the English sample to 300 seconds. It is a synthetic length diagnostic, not an independent five-minute meeting or a diversity test.
- Hardware was NVIDIA GeForce RTX 4090, driver 610.57.04. The installed CUDA package was 13.3.1-1. cuDNN 9.25.0.15-1 was extracted beneath the isolated evidence directory and exposed through the comparison process's `LD_LIBRARY_PATH`; this was not a host cuDNN installation.
- Both providers ran the same CUDA-capable debug engine, SHA-256 `738457f01c2d59864a680a5aea8493147ad6a49e030af65f81c27ef3546fc9b5`, with the existing `diarize/diarization` segmentation and embedding models. The binding pins sherpa-onnx 1.13.7; its CUDA archive carries ONNX Runtime 1.27.1 for CUDA 13 and cuDNN 9.
- The comparison driver alternated CPU-first and CUDA-first order across the three repeats. Each invocation launched a fresh process. The default engine thread count resolves to the core count capped at four. Results are end-to-end cold-process measurements, not warm-session inference-only timings or an optimized release-build claim.

## Build and installed-path checkpoint

`make build test lint` passed with exit 0 at commit `a3077006fada494612b71743fbf9ee3a26cb18cd`, including all 74 Qt tests and the CUDA Models-row coverage. The log is `/tmp/dtv-gate-fn42-final.log`; `.flow/tmp/green-receipts/a3077006-unittest.json` binds its command hash and successful gate receipt to that commit. This is the executable-change checkpoint before this documentation update. It does not change the failed R2 performance result.

An ephemeral overlay/mount-namespace check exercised installed-path discovery with and without the CUDA drop-in. Both doctor commands exited 0. With the drop-in, doctor selected `/usr/lib/dettivo/engines-cuda/dettivo-engine-diarize`; without it, doctor selected `/usr/lib/dettivo/engines/dettivo-engine-diarize`. Direct CLI inference at those selected paths reported CUDA and CPU respectively and returned the same six turns.

Both doctor rows had `backend = null` and `running = false`. They prove path selection before load; the separate CLI inference proves the backends. The CPU tree was a matching debug stage, not a full CPU release-package installation. No host `/usr` mutation or package installation occurred. This is scoped R3 discovery/removal evidence, not overall desktop health or installed-session approval.

Receipts are `/home/gordon/work/dettivo-linux-wt/_factory/fn42-runtime-ncbopZ/installed-68o2d6em/installed/result.json` and the sibling `removed/result.json`; each directory also retains `doctor.stdout`, `engine.stdout` and `daemon.log`. The companion JSON includes both result payloads. CUDA packaging passed its seven-file manifest, and `makepkg` completed with `namcap` warnings retained in `/tmp/fn42-package-cuda-verified.log` and `/tmp/fn42-makepkg-cuda-verified.log`; those checks did not install the package.

## Durable and raw evidence

The [machine-readable companion](diarization-cuda-2026-09-09.json) retains unrounded comparison results, per-run times, fixture hashes, engine PIDs, sample counts and the canonical benchmark answer. It is a task-specific report; the general benchmark schema and its published table are unchanged.

Raw local receipts remain under `/home/gordon/work/dettivo-linux-wt/_factory/fn42-runtime-ncbopZ/`:

- `compare-e3wd9bz4/report.json` contains the three-repeat `gold`, `public-en` and `public-zh` comparisons.
- `compare-wymrgxou/report.json` contains the one-thread `gold` diagnostic.
- `compare-3649ufkm/report.json` contains the 300-second loop comparison.
- Each comparison directory holds per-invocation `.stdout` and `.stderr` files; its report contains the full turns and GPU samples.
- `compare.py` is the local comparison driver. `cudnn.pkg.tar.zst` retains the inspected runtime package metadata.
- `/home/gordon/work/dettivo-linux-wt/_factory/fn42-canonical/run-1788936876-171032/diarization/diarization-report.json` is the canonical receipt copied into the companion, SHA-256 `d7d5d8cf867f04d1c87586e70db6bfd5750c0978dc2b0a956d1fc6ab4cf57b43`. `/tmp/fn42-canonical-pipeline.log` contains the identical answer.

The result blocks the performance acceptance criterion in [ADR 0057](../../adr/0057-cuda-diarization-drop-in-with-cpu-fallback.md). It does not establish installed-session QA, package installation/removal acceptance, or release approval. No performance threshold was weakened to accept this result.
