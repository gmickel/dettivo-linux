# CUDA convolution tuning helps, but still misses 4x CPU throughput

The source-built CUDA candidate reduced the original CUDA wall times, but R2 remains failed. On the 300-second English loop, CUDA fell from 37.198 to 20.921 seconds and reached 1.310x CPU throughput. The target is 4x. Gold and Chinese samples remained slower than CPU. This report preserves the [original negative result](diarization-cuda-2026-09-09.md) and records the separate tuning attempt; neither result authorizes shipping.

## Cause and the bounded change

Variable input shapes repeatedly incur cuDNN frontend planning work. ONNX Runtime 1.27.1's `Conv::UpdateState` compares the latest dimensions with its saved dimensions. A change calls `CreateCudnnFeExecutionPlan`, which creates a new graph and validates and builds its execution plans; unchanged dimensions reuse that state. Search mode 1 selects cuDNN heuristic A; mode 2 selects FALLBACK. These are the mechanisms in the [pinned CUDA convolution source](https://github.com/microsoft/onnxruntime/blob/v1.27.1/onnxruntime/core/providers/cuda/nn/conv.cc).

The research shape test supports that diagnosis. Twenty alternating 48,000/64,000-sample embeddings took 1.265 seconds on CUDA; grouping the same shapes took 0.208 seconds. CPU took 0.575 and 0.554 seconds respectively. Fixed-shape median calls were 4.928 ms on CUDA and 31.201 ms on CPU. The fixed CUDA sequence's first call still cost 288.560 ms. These are isolated embedding diagnostics, not full-diarization throughput or a shipped reordering of speaker turns.

The implementation changes one sherpa session option from `OrtCudnnConvAlgoSearchHeuristic` to `OrtCudnnConvAlgoSearchDefault`. It rebuilds only the CUDA C API from pinned sherpa-onnx 1.13.7 source and retains ONNX Runtime 1.27.1. This lowers planning cost on the measured workload; it does not add shape caching or eliminate shape changes. Models, precision and stride remain unchanged, and CPU packaging still uses its prebuilt archive.

Hook-based option trials, no-spin trials and exhaustive-search trials were diagnostic experiments only. The production-path measurements below used the source-built library with an empty `diagnostic_environment` receipt, without `LD_PRELOAD` or API interception. No diagnostic runtime mechanism is part of the implementation.

## Actual candidate results

Cold-process wall times include load and inference. Three-repeat rows use each provider's median; the loop and automatic-speaker rows have one run per provider. Threads use the existing default. Speedup means CPU wall time divided by CUDA wall time.

| Sample and speaker mode | Runs per provider | Original CUDA seconds | Tuned CPU seconds | Tuned CUDA seconds | Tuned CUDA / CPU throughput |
|---|---:|---:|---:|---:|---:|
| `gold`, known two | 3 | 2.619 | 1.196 | 1.568 | 0.763x |
| `public-en`, known two | 3 | 7.182 | 4.608 | 4.086 | 1.128x |
| `public-zh`, known four | 3 | 6.676 | 3.065 | 4.092 | 0.749x |
| `public-en-loop-300s`, known two | 1 | 37.198 | 27.403 | 20.921 | 1.310x |
| `gold`, automatic | 1 | not measured | 1.164 | 1.574 | 0.740x |
| `public-en`, automatic | 1 | not measured | 4.673 | 4.089 | 1.143x |
| `public-zh`, automatic | 1 | not measured | 3.067 | 4.117 | 0.745x |

All 26 candidate comparison invocations reported the requested backend and exited successfully. All seven comparisons retained exact CPU/CUDA turn parity, including the automatic-speaker mode. All 13 CUDA invocations had GPU samples naming their exact engine PID. Matching public-sample outputs establishes provider parity, not ground-truth diarization accuracy.

The 300-second input remains a loop of the 54.805-second official English sample, not an independent meeting. Gold is the repository's 18.856-second two-Piper-voice fixture; the official Chinese sample is 56.861 seconds. Their source URLs and original provenance are in the [first report](diarization-cuda-2026-09-09.md#inputs-and-environment); the companion retains their SHA-256 values.

## Canonical acceptance rerun

The fresh `dettivo-qa pipeline diarization --bench` run returned exit 1 and verdict `fail`. CPU and CUDA both found two speakers with DER `0.020396270396270396`. CPU took 1,203 ms and CUDA 1,696 ms, yielding 0.709x CPU throughput. The relative 4x requirement remains unmet; the unchanged quality score does not override it.

The receipt is `/home/gordon/work/dettivo-linux-wt/_factory/fn42-tuned-canonical/run-1788940893-675212/diarization/diarization-report.json`, also captured in `/tmp/fn42-tuned-canonical.log`. The companion preserves the complete canonical answer. This rerun used no `LD_PRELOAD` or `FN42_CUDNN_SEARCH` override.

## Loaded identity matters

The Rust wrapper SHA-256 stayed `738457f01c2d59864a680a5aea8493147ad6a49e030af65f81c27ef3546fc9b5`. It cannot distinguish this candidate from the original run because the changed code is in a dynamic library.

| Loaded component | SHA-256 |
|---|---|
| Patched `libsherpa-onnx-c-api.so` | `75da8de18cfd77443ca4dee955228feeacdd5b9c97edaefac601a1ca4bf05522` |
| Unchanged `libonnxruntime.so` | `bd6591e07bac04657dddd92a4407f4b5aab8268e25b55d901e886648eeb7b8f6` |
| sherpa-onnx 1.13.7 source archive | `ee0c20cafb34cc1f86afb2845babd941c26e46de4a9925cbe86fd55ff3557818` |

All three candidate receipts name these loaded library hashes and paths. The build uses matching individually checksummed ONNX Runtime 1.27.1 headers. The CUDA C API builds in release mode even under the debug Rust wrapper. The environment is the same RTX 4090, driver 610.57.04, CUDA 13.3.1 and process-local extracted cuDNN 9.25.0.15 recorded in the original experiment.

## Sampled process GPU memory

Existing `nvidia-smi` samples show a run-set maximum of 838 MiB before tuning and 1,084 MiB after tuning, an increase of 246 MiB observed on the English sample. Each entry below is the largest sampled memory value for that run's exact engine PID. Values are MiB; repeat order is 0, 1, 2.

| Sample | Original per-run maxima | Tuned per-run maxima | Original maximum | Tuned maximum |
|---|---|---|---:|---:|
| `gold` | 706, 826, 706 | 698, 824, 824 | 826 | 824 |
| `public-en` | 834, 838, 834 | 1,084, 1,080, 844 | 838 | 1,084 |
| `public-zh` | 834, 826, 826 | 840, 840, 828 | 834 | 840 |

These are sampled process GPU-memory observations, not allocator peaks. Sampling may miss transients, and the two run sets do not isolate the patch's causal effect on memory. The English result identifies a possible additional 246 MiB tradeoff to investigate; it does not establish a fixed per-run increase. The companion preserves PID, repeat, sample count and maximum for every run, derived from `compare-e3wd9bz4/report.json` and `compare-c417ka1l/report.json`. No new GPU run was needed.

## Build and packaging checkpoint

The rebuilt CUDA package and `makepkg` checks returned exit 0; logs are `/tmp/fn42-package-cuda-tuned.log` and `/tmp/fn42-makepkg-cuda-tuned.log`, including retained `namcap` warnings. The staged release C API hash is `d6f036763cd77cb6315776f4364eaf42cbf177fd54e8f34e30871a637ca32ebc`, distinct from the benchmark's debug-build-directory C API hash above. The packaged engine and rebuilt C API use `$ORIGIN` only; the three upstream ONNX Runtime libraries remain byte-identical.

The fresh ephemeral overlay/mount-namespace check passed both phases. Doctor exited 0 and selected `/usr/lib/dettivo/engines-cuda/dettivo-engine-diarize` with the drop-in, then `/usr/lib/dettivo/engines/dettivo-engine-diarize` without it. Direct inference at the selected paths reported CUDA and CPU with the same six turns. Doctor itself reported unloaded engines with null backends, so it proves path selection, not loaded-provider state. No host `/usr` mutation or package installation occurred.

Receipts are `/home/gordon/work/dettivo-linux-wt/_factory/fn42-runtime-ncbopZ/installed-mqt52s9s/installed/result.json` and the sibling `removed/result.json`; the companion preserves both. R2 remains failed regardless of packaging and discovery checks.

`make build test lint` passed with exit 0 at executable commit `bbb5e1702e6a270ecbf4b975b67546430168b3f4`, including Rust build/tests, clippy, lints, docs and all 74 Qt tests. The log is `/tmp/dtv-gate-fn42-tuning-final.log`; `.flow/tmp/green-receipts/bbb5e170-unittest.json` binds the successful receipt and command hash to that commit. This subsequent report update is documentation-only and records existing gate and memory evidence without code or GPU changes. A green development gate leaves the measured 4x performance failure unchanged.

## Evidence and bounds

The [JSON companion](diarization-cuda-tuning-2026-09-09.json) retains unrounded comparisons, per-run timings, input hashes, loaded-library identities, diagnostic-environment records, PID-proof counts and shape diagnostics.

Production-path receipts under `/home/gordon/work/dettivo-linux-wt/_factory/fn42-runtime-ncbopZ/`:

- `compare-c417ka1l/report.json` contains three repeats of each short sample with known speaker counts.
- `compare-xnxz16bv/report.json` contains the 300-second loop.
- `compare-zk2aanv6/report.json` contains automatic-speaker-count comparisons.
- Their sibling stdout/stderr files and full GPU samples remain local raw evidence.

Research-only evidence lives under `/home/gordon/work/dettivo-linux-wt/_factory/fn42-tuning-c8ldde/`: `embedding-shapes-wide-cpu.json`, `embedding-shapes-wide-cuda.json`, `emb-profile_2026-09-09_09-29-05_336.json` and `seg-profile_2026-09-09_09-29-05_192.json`. The profiles name CUDA execution for embedding and segmentation nodes; the source inspection and controlled shape experiments explain the repeated planning cost.

This is a measured improvement over the first CUDA attempt and a failed 4x acceptance result. It does not establish full installed-session QA, a completed release gate or a shipping decision.
