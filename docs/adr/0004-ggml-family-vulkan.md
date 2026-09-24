# 0004. One ggml family with Vulkan by default, Sherpa-ONNX only for diarization

Status: Accepted 2026-09-03; the Parakeet-for-meetings expectation is settled by [ADR 0018](0018-parakeet-engine-dictation-only.md): the spike measured its timestamps outside the merger's tolerance, so Parakeet is dictation-only; amended 2026-09-06 by 0049 (the strict Vulkan preference refuses instead of loading on the CPU)

## What this gives you

Whisper, Parakeet and the language model all accelerate on NVIDIA, AMD and Intel GPUs from one installed package, and Parakeet returns word timestamps. (Whether those timestamps were precise enough for meetings was left to a spike; ADR 0018 records that they were not, so meetings keep Whisper.)

## Situation

ONNX Runtime has no Vulkan execution provider on Linux; with it, AMD and Intel users run Parakeet on the CPU, and NVIDIA needs a separate CUDA build, which is why Voxtype's AUR package ships variant binaries. parakeet.cpp (mudler, C API, pushed 28 August 2026) runs every offline Parakeet family on ggml with CPU, Vulkan and CUDA backends, publishes GGUF quantizations, reports word timestamps and confidence, and documents WER-0 parity with NeMo. whisper.cpp and llama.cpp already run on the same ggml backends. Speaker diarization has no ggml implementation; the pyannote segmentation plus 3D-Speaker embedding pipeline on Sherpa-ONNX is proven on Windows and weighs about 47 MB.

## Decision

Speech and language engines use ggml: whisper.cpp through `whisper-rs`, parakeet.cpp through an in-repo `parakeet-cpp-sys` binding to `parakeet_capi.h`, llama.cpp through `llama-cpp-2`. Each engine tries Vulkan first and falls back to CPU, and reports which it chose. Sherpa-ONNX runs diarization only, on the CPU, inside its own engine process.

## Consequences

- Parakeet becomes meeting-capable on Linux if the S-13 spike confirms timestamp precision against a golden alignment; both other ports currently gate Parakeet out of meetings for lack of timestamps. The spike ran and did not confirm it: ADR 0018 keeps Parakeet dictation-only.
- The default build needs Vulkan headers and shaderc at build time and `vulkan-icd-loader` at run time; CPU fallback covers machines without a Vulkan device.
- parakeet.cpp is young and single-maintainer; the binding is pinned and owned in this repository, and Sherpa-ONNX stays documented as the fallback for Parakeet.
- ggml is compiled three times, once per engine binary, which lengthens builds; a shared ggml build is a later optimisation.
