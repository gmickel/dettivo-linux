# Diarization on the GPU: the CUDA build of the diarize engine as a drop-in, with the CPU build as the fallback

## Conversation Evidence

> user (2026-09-06): "is everything gpu accelarated that can ben?" then "do some research into a gpu path for the diarization while we wait perhaps we can flesh out the masterplan/a spec"
> masterplan T6: "Sherpa-ONNX only for diarization, CPU | Port diarization to ggml; ONNX Runtime for everything | No ggml diarization exists; the pyannote plus 3D-Speaker pipeline is proven on Windows and small (about 47 MB); CPU is fine for a post-meeting pass. Isolated in its own engine process so ONNX Runtime never touches the daemon."
> masterplan T5: "ONNX Runtime has no Vulkan path on Linux, so AMD users would be CPU-only."
> ADR 0034: the daemon searches `[engines] directory`, then `/usr/lib/dettivo/engines-cuda`, then `/usr/lib/dettivo/engines`; "Engine binaries are separate files so a later `dettivo-engines-cuda` package can replace them."
> ADR 0035 bounds: "the pass runs on the CPU only (T6)"; fn-36's meetings pack measured diarization at 13.6x realtime on the CPU (NFR-4 asks 4x).
> Research 2026-09-06: ONNX Runtime has no Vulkan execution provider; the WebGPU provider is a plugin marked experimental. The CUDA provider pairs ONNX Runtime 1.27 to 1.29 with CUDA 13.0 and cuDNN 9, and a build against CUDA 13.0 runs on any 13.x toolkit; cuDNN 8 and 9 builds are not interchangeable. sherpa-onnx accepts `provider = "cuda"` in the segmentation and embedding model configs and ships `sherpa-onnx-v1.13.7-cuda-13.x-cudnn-9.x-onnxruntime1.27.1-linux-x64-gpu.tar.bz2` (218 MB) beside the CPU archive; the pinned v1.12.9 has only a `linux-x64-gpu` archive built for CUDA 12. Arch ships `cuda` 13.3 and `cudnn` 9.25 (`cudnn` depends on `cuda>=13`, 814 MB installed); thor has `cuda` 13.3.1 installed and no `cudnn`. pyannote 3.1 reports about 92x realtime on an RTX A6000 against 13.6x here on the CPU.


## Delivery decision (2026-09-10)

Gordon explicitly requested delivery of fn-42 and fn-56 in their current state to synchronize main. The criteria below govern this delivery only. The original criteria and every failed measurement remain historical evidence below; completion records accepted limitations, never a benchmark pass. This delivery covers the existing code and documentation, with no new model experiments, host installation, release tag, or public release.

## Acceptance Criteria

- **R1:** Deliver the implemented optional CUDA diarization build, actual-provider reporting, prerequisite CPU fallback and strict CUDA refusal, preserving the default CPU package and existing regression evidence.
- **R2:** Retain the original failed fourfold-throughput criterion and all CPU/CUDA benchmark evidence. Gordon accepts the current implementation for delivery despite this miss; no 4x speedup or universal CUDA advantage is claimed.
- **R3:** Deliver the existing CUDA package manifest, AUR recipe and isolated installation/selection checks. Main synchronization does not install the package on the live host or publish a release.
- **R4:** Keep configuration, user documentation and ADR 0057 consistent with the accepted optional implementation. Run the repository build/test/lint gate on the delivery code and preserve the measured limitations.

## Goal & Context

<!-- Goal & Context: 40% [user], 30% [paraphrase], 30% [inferred] -->

The speaker pass keeps up with the rest of the meeting lane on an NVIDIA machine: the post-meeting diarization that takes four minutes for an hour of audio on the CPU takes seconds on the GPU, and it no longer competes with the language-model analysis for cores while both run after a meeting. The path is the one the masterplan reserved: ONNX Runtime's CUDA provider behind the existing engine process, shipped as the `dettivo-engines-cuda` drop-in that ADR 0034 already searches first, with the CPU build staying the default everywhere else. Nothing changes for AMD or Intel machines, which keep the CPU pass (ONNX Runtime has no Vulkan provider), and nothing touches the daemon: the engine process is the only thing that links ONNX Runtime.

## Architecture & Data Models

<!-- Architecture & Data Models: 40% [paraphrase], 60% [inferred] -->

- `crates/sherpa-onnx-sys` learns a `cuda` feature: it fetches the pinned `cuda-13.x-cudnn-9.x` archive of the same sherpa-onnx release as the CPU archive (both pins move together, to 1.13.7 or later, checksummed in `build.rs`), keeps `libsherpa-onnx-c-api.so`, `libonnxruntime.so` and the `libonnxruntime_providers_cuda.so` and `_shared.so` provider libraries, and links against them; the CPU feature stays the default. [inferred]
- `crates/dettivo-engine-diarize` gains `--provider auto|cpu|cuda` (the engine's own flag, mirrored by `[engines.diarize] backend = "auto"` like the other three engines' `backend` keys): `auto` tries `cuda` when the binary carries the provider and `libcudart.so.13`, `libcudnn.so.9` and the provider libraries resolve, and falls back to `cpu` with the reason on `load`'s answer (`backend`, `fallback_reason`), so a missing cuDNN never fails a meeting. `load` reports `backend` the way the Whisper engine does, `speech.engines` and `dettivo doctor` show it, and the meetings pack's `diarization_throughput` records it. [inferred]
- Packaging (ADR 0034): `scripts/package.sh --cuda` stages a second tree with only `usr/lib/dettivo/engines-cuda/dettivo-engine-diarize` and its libraries; `packaging/aur/dettivo-engines-cuda/PKGBUILD` depends on `dettivo-bin` (or `dettivo`), `cuda` and `cudnn`, and the manifest check learns the second manifest. The daemon's search order needs no change. Uninstalling the package returns the CPU engine. [inferred]
- Config and docs: `[engines.diarize] backend` on the Models settings route beside `threads`; `docs/engines.md`, `docs/models.md`, `docs/install.md` and `docs/config.md` describe the drop-in and the fallback; ADR 0057 records the choice (CUDA drop-in over a Vulkan path that does not exist, the release-pin pairing, the fallback rule) and updates 0035's bound. [inferred; 0044 was already occupied when implementation began]
- QA: `dettivo-qa pipeline diarization --bench` and the meetings pack's `diarization_throughput` record `backend`; on thor the `gpu_workload_proof` step of the meetings pack extends to the diarize engine's pid; a fixture test proves the fallback by pointing the engine at a directory without the provider libraries. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Engine protocol: `load` takes `provider` (`auto|cpu|cuda`, default `auto`) beside `threads`; its answer carries `backend` (`cpu|cuda`) and `fallback_reason?`. `speech.engines` rows carry `backend` for the diarize engine as they do for the others. [inferred]
- Config: `[engines.diarize] backend = "auto"` (Linux addition, registered). [paraphrase]
- No change to `meetings.diarize`, the speaker methods or the row model. [paraphrase]

## Historical acceptance criteria (retained unchanged)

- **R1:** `cargo build -p dettivo-engine-diarize --features cuda` produces an engine that reports `backend = "cuda"` on `load` on a machine with `cuda` and `cudnn` installed, and the same binary reports `backend = "cpu"` with a `fallback_reason` naming the missing library on a machine without them; a unit test proves the fallback with the provider libraries hidden. Errors: `--provider cuda` on a machine without the libraries fails `load` naming the library, `auto` never fails for that reason. [inferred]
- **R2:** On thor, `dettivo-qa pipeline diarization --bench` on the CUDA engine reports the same DER (0.020 with two speakers) at a realtime factor at least four times the CPU figure, and the meetings pack's `diarization_throughput` records `backend = "cuda"` with the engine's pid in the GPU proof. Errors: a DER that moves fails the pipeline naming both figures. [inferred]
- **R3:** `scripts/package.sh --cuda` builds the `engines-cuda` tree against its own manifest, `packaging/aur/dettivo-engines-cuda` builds under `makepkg` with `namcap`, installing it beside `dettivo-bin` makes `dettivo doctor` name the CUDA diarize engine and removing it returns the CPU one. Errors: a manifest mismatch fails by path. [inferred]
- **R4:** `[engines.diarize] backend` is in the schema, the default file, `docs/config.md`, `config print-default` and the Models settings route; `docs/engines.md`, `docs/models.md` and `docs/install.md` document the drop-in and the fallback; ADR 0057 is indexed and ADR 0035's bound is updated. Errors: `just docs` and the settings key lint fail by name. [inferred; ADR number corrected because 0044 was occupied]

## Boundaries

- No Vulkan or WebGPU provider: ONNX Runtime has none that is supported; AMD and Intel keep the CPU pass, and a ggml port of the pipeline stays out of scope.
- No CUDA builds of the ggml engines: Whisper, Parakeet and the language model stay on Vulkan; the `engines-cuda` drop-in carries only the diarize engine until a measured need says otherwise.
- No live diarization; the pass stays post-meeting.
- The release pin moves for both archives together; the CPU path's behaviour does not change.
