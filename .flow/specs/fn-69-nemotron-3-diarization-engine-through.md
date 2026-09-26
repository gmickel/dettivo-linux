# Nemotron 3 Diarization engine through NeMo-Speech.cpp

## Goal & Context

fn-64 measured NVIDIA Nemotron-3-Diarization against the current sherpa-onnx engine. It had about a fifth of the AMI confusion at a quarter of the CPU time, and the biggest proxy gain was on German meetings. Gordon has read and accepted its OpenMDW 1.1 licence. This spec makes it a product engine.

Use what the repo already has. Speech engines run on ggml (ADR 0004): whisper.cpp, and parakeet.cpp through the in-repo `parakeet-cpp-sys` binding, built from pinned source with CPU, Vulkan and CUDA. NVIDIA's NeMo-Speech.cpp is the reference runtime for this model: Apache-2.0, ggml, stable C headers and shared libraries, CPU, CUDA and Vulkan. NVIDIA ships the model as `Nemotron-3-Diarization.q8_0.gguf`. A `nemo-speech-cpp-sys` crate on the `parakeet-cpp-sys` pattern keeps the Vulkan path for AMD and Intel GPUs, which ONNX Runtime lacks on Linux. The ONNX route through `ort` (as omarchy-meeting-recorder does) is the fallback if NeMo-Speech.cpp's C API or accuracy parity falls short.

## Approach

- A pinned `nemo-speech-cpp-sys` crate: checksum-verified source fetch and a CMake build, like `parakeet-cpp-sys`, with ggml pinned. It ships as a separate engine process, so its ggml copy never meets another engine's.
- Diarization runs in its own engine process: either `dettivo-engine-diarize` gains a Nemotron backend, or a sibling engine binary. Choose whichever keeps files under 500 lines and the crate dependency edges clean. It speaks the existing engine protocol. It returns turns, plus per-frame speaker probabilities for fn-70.
- The model is a catalogue entry downloaded like the other models. `[meetings.diarization] model` selects it. The backend follows `[engines.diarize] backend` (auto, cpu, cuda, plus vulkan).
- Long meetings are chunked as the runtime documents. Above 8 speakers, the speaker pass falls back to the sherpa-onnx engine. An expected participant count, where known, caps the output.
- A parity check against the fn-64 ONNX port and the transformers reference implementation on the same audio (probabilities or turns), so accuracy parity is proven, not assumed.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** A Nemotron engine runs from the product on CPU, CUDA and Vulkan on Thor. It is selected by `config.toml` and downloaded through the model catalogue, and `dettivo doctor` reports its backend.
- **R2:** Its turns match the fn-64 reference port within a stated tolerance on the AMI test meetings. The tolerance and the result are recorded in the report.
- **R3:** On the fn-67 bench with the fn-68 rule, Nemotron's headline attribution error on AMI dev and on the local German meetings is reported against the current engine's, with CPU and CUDA wall times.
- **R4:** Nemotron becomes the default diarization model if R3 shows it ahead of the current engine on the pooled headline number and not behind on German, both within the bench's intervals. Otherwise it ships opt-in. An ADR records the decision with the numbers.
- **R5:** Meetings over 8 speakers, missing models, and a failed GPU load each fall back with a stated reason, and tests cover them.
- **R6:** `docs/engines.md`, `docs/config.md` and `docs/meetings.md` describe the engine, its licence (OpenMDW 1.1), its backends and the fallback.

## Boundaries

- The sherpa-onnx engine stays available and keeps working.
- No change to the assignment rule (fn-68) or the voice check (fn-70) here.
- Packaging follows the existing engine packaging. No release in this spec.

## Decision Context

Gordon approved recommendation 2 on 2026-09-26: "see if we can use what we already have". The repo has no `parakeet-rs`; it binds parakeet.cpp (ggml) directly. NeMo-Speech.cpp is the same kind of runtime, from NVIDIA, for this exact model.
