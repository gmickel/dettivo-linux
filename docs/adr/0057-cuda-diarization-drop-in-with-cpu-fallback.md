# 0057. CUDA diarization is an optional drop-in with CPU fallback

Status: Accepted for delivery with the measured performance limitation by Gordon on 2026-09-10. Amends 0034 and 0035.

## What this gives you

NVIDIA machines can run the post-meeting speaker pass on CUDA while the default package keeps CPU diarization. The optional drop-in uses the existing 47 MB model set and reports the provider it actually loaded. Automatic selection falls back to CPU with the reason when CUDA prerequisite checks fail, so a missing runtime library does not prevent a meeting's speaker pass.

## Situation

[0034](0034-install-layout-cuda-drop-in-and-release-workflow.md) reserved `/usr/lib/dettivo/engines-cuda` ahead of the normal engine directory. [0035](0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md) isolated sherpa-onnx and ONNX Runtime in the diarization process and chose CPU inference. The existing ONNX models also run through ONNX Runtime's CUDA provider. ONNX Runtime has no supported Vulkan provider for this path; the ggml engines keep their existing Vulkan backend.

## Decision

- Pair both sherpa-onnx archives at 1.13.7 and pin each checksum in `sherpa-onnx-sys`. Its `cuda` feature selects the CUDA 13, cuDNN 9, ONNX Runtime 1.27.1 archive and retains the CUDA and shared provider libraries alongside the C API and ONNX Runtime libraries. CUDA and cuDNN remain runtime package dependencies.
- For the tuning retry, rebuild only the CUDA sherpa C API from the checksummed 1.13.7 source. Apply the one-line `cuda-conv-default.patch` with zero fuzz, replacing `OrtCudnnConvAlgoSearchHeuristic` with `OrtCudnnConvAlgoSearchDefault` before CMake builds it. The ONNX Runtime 1.27.1 binary and providers stay from the verified GPU archive; five matching headers are checksummed individually in `cuda_build.rs`. CPU continues using its prebuilt archive. This adds CMake, Ninja, a C++ compiler and GNU patch to the CUDA build requirements, with no runtime interposition or change to models, stride or precision. Candidate results must be recorded separately from the original negative report.
- Add `--provider auto|cpu|cuda` and the matching `[engines.diarize] backend = "auto"` setting. A separate config enum keeps CUDA out of the ggml engines' backend choices. Settings / Models edits the key beside the thread count.
- Automatic selection finds provider libraries beside the loaded ONNX Runtime, checks their files and dependencies with glibc `ldd`, verifies the CUDA factory symbol and checks `cudaGetDeviceCount`. Failed prerequisites select CPU. ONNX Runtime loads the providers itself; manually loading their libraries can run constructors before their host is initialized. Later model initialization failures and GPU out-of-memory errors have no automatic CPU recovery. `load` reports `backend` and optional `fallback_reason`. An explicit `cuda` preference fails the load naming the unavailable dependency; `cpu` and `DETTIVO_FORCE_CPU=1` force CPU.
- Package the diarization engine, its four libraries, `LICENSE` and `NOTICE.md` in `dettivo-engines-cuda`, checked against a separate seven-file manifest. The existing search order selects it. Removing it returns future engine starts to the CPU engine unless a configured directory overrides discovery.
- Record the actual backend in the diarization benchmark and meetings throughput evidence. A CUDA benchmark compares the same audio and model against an explicit CPU run; matching DER and speaker count and at least four times the CPU realtime factor are the acceptance target. CUDA workload proof must identify the diarization process itself.

## Consequences

Gordon requested delivery of the current fn-42 and fn-56 implementation to main on 2026-09-10. The CUDA package remains optional and CPU remains the default package. This decision accepts the measured speed limitation for delivery; it does not turn the original fourfold-throughput failure into a pass or authorize a public release. Further speed work requires a separately scoped change.

[ADR 0058](0058-strict-diarization-accuracy-evaluation.md) extends the source build to CPU and CUDA for calibrated clustering. The build module is now `source_build.rs`; the earlier measurements below retain their original code and model identities.

The [RTX 4090 experiment](../reports/benchmarks/diarization-cuda-2026-09-09.md) failed the original required 4x CPU throughput despite matching turns and proving the CUDA engine's GPU PID. CUDA was slower at default threads on the short samples and the 300-second loop. That result initially blocked delivery; the explicit acceptance above supersedes the delivery block while preserving the failed measurement.

The [source-built tuning retry](../reports/benchmarks/diarization-cuda-tuning-2026-09-09.md) reduced CUDA's 300-second-loop wall time from 37.198 to 20.921 seconds, reaching 1.310x CPU throughput with exact turn parity. The one-line convolution-search change reduces the measured planning cost from variable input shapes, but still misses 4x. Automatic-speaker comparisons also preserve CPU/CUDA parity. The retry retains the failed acceptance outcome and leaves the original negative report intact.

The daemon never links ONNX Runtime, and this package adds no CUDA backend to Whisper, Parakeet or the language model. AMD and Intel retain CPU diarization. The pass remains post-meeting, and cancellation retains 0035's bound while sherpa-onnx finishes its current pass.

Runtime discovery and CUDA speed are separate checks. CPU fallback proves missing libraries do not break automatic selection; only measured GPU execution can establish the speed target. Installation, removal and hardware performance need their own evidence and cannot be inferred from a successful package build.
