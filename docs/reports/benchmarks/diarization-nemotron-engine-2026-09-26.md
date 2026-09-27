# Nemotron 3 Diarization as a product engine

This report tells whoever sets the speaker pass's default what `dettivo-engine-nemotron` does against the current sherpa-onnx engine, and whether it reproduces the pipeline fn-64 measured. The engine matches fn-64's ONNX reference port within the tolerance fixed before the first run on every AMI test meeting, on the CPU, on Vulkan and on CUDA. On the fn-67 bench under this branch's labelling rule, it puts the wrong or no speaker on 6.5 points fewer AMI dev words than the current engine and ties it on the held-out AMI test meetings. It leaves 2.2 points more German remote lines without a speaker, so by fn-69's R4 rule it ships opt-in ([ADR 0073](../../adr/0073-nemotron-3-diarization-runs-through-nemo-speech-cpp.md)). It diarizes an hour of meeting in 12 seconds with CUDA, 15 seconds with Vulkan and 108 seconds on the CPU, against 265 seconds for the current engine on the CPU.

The [machine-readable receipt](diarization-nemotron-engine-2026-09-26.json) holds every pooled figure, the paired intervals, the per-meeting parity rows for AMI, the timings, the runtime commits and the binary and model hashes. Nothing in it names a retained meeting.

## What ran

- **The engine.** `dettivo-engine-nemotron` at tree `f584158` plus this change, built three ways: CPU only, ggml Vulkan, and ggml CUDA for compute capability 8.9 (the package build keeps ggml's default architecture list). It runs NeMo-Speech.cpp at commit `97a15afa` on stock ggml with the catalogue's `Nemotron-3-Diarization.q8_0.gguf` (revision `f667ed7`), the reference streaming geometry and fn-64's recorder post-processing.
- **The reference.** `scripts/qa/nemotron3/nemotron3_diarize.py` with the fp32 ONNX graph from `onnx-community/Nemotron-3-Diarization-ONNX`, on ONNX Runtime's CPU provider. fn-64 matched that port to the transformers implementation of `nvidia/Nemotron-3-Diarization` within 1.1e-5 on the first 600 s of ES2004a, so parity with the port carries over to the transformers reference.
- **The host.** AMD Ryzen 9 7950X3D (16 cores, 32 threads), NVIDIA GeForce RTX 4090, CUDA 13.3.1, Vulkan loader 1.4.357, Arch Linux.

## Parity with the reference port

`scripts/qa/nemotron3/parity_engine.py` runs both on the same AMI test audio and compares the 10 ms probabilities and the turns, both taken through the same post-processing. The tolerance, fixed before any run, is per meeting: turn-level DER against the port at most 2.0%, confusion at most 0.5%, and at most 1% of speech decisions (probability over 0.5, per frame and channel) flipped.

| AMI test meeting | Backend | Decisions flipped | Turn DER vs port | Confusion vs port | Speakers, engine / port |
|---|---|---:|---:|---:|---:|
| ES2004a | CPU | 0.045% | 0.40% | 0.001% | 4 / 4 |
| TS3003a | CPU | 0.038% | 0.61% | 0.000% | 2 / 2 |
| IS1009b | CPU | 0.046% | 0.24% | 0.041% | 4 / 4 |
| EN2002c | CPU | 0.087% | 0.90% | 0.005% | 3 / 3 |
| ES2004a | Vulkan | 0.054% | 0.56% | 0.003% | 4 / 4 |
| TS3003a | Vulkan | 0.040% | 0.66% | 0.000% | 2 / 2 |
| IS1009b | Vulkan | 0.051% | 0.28% | 0.061% | 4 / 4 |
| EN2002c | Vulkan | 0.081% | 0.81% | 0.001% | 3 / 3 |
| ES2004a | CUDA | 0.050% | 0.43% | 0.000% | 4 / 4 |
| TS3003a | CUDA | 0.043% | 0.71% | 0.000% | 2 / 2 |
| IS1009b | CUDA | 0.041% | 0.31% | 0.001% | 4 / 4 |
| EN2002c | CUDA | 0.069% | 0.74% | 0.002% | 3 / 3 |

Every row is within the tolerance. The largest single-frame probability difference is 0.26 to 0.56 depending on the meeting, and the mean is 0.0007 to 0.0048. The differences come from the q8_0 weights against fp32 and from NeMo-Speech.cpp's own features, and they flip under 0.09% of decisions. Scored against the AMI references under ADR 0058, the engine's confusion is within 0.1 point of the port's on every meeting and backend (for example 2.37% against 2.46% on TS3003a, where both find two speakers of four), and its DER within 0.15 point.

## The bench

The fn-67 bench scored the product's own speaker rule (`crates/dettivo-meeting/src/diarize.rs` on this branch, before fn-68's change) on each engine's turns. The Nemotron column is the CUDA build. Parity shows the CPU build's turns within 0.9% turn DER of it on AMI test.

| Attribution error (headline) | Current engine | Nemotron, product engine | Delta, paired 95% interval |
|---|---:|---:|---:|
| AMI dev, 18 meetings, 580 min | 28.99% | 22.46% | −6.54 [−11.00, −2.44] |
| AMI test, 4 meetings, 126 min, held out | 22.98% | 21.98% | −1.00 [−2.65, +1.48] |

| Retained meetings, proxies | Current engine | Nemotron, product engine | Delta, paired 95% interval |
|---|---:|---:|---:|
| German (6, 278 min), remote lines unlabelled | 11.06% | 13.26% | +2.20 [+1.18, +4.42] |
| German, local/remote proxy (mix) | 0.90% | 0.14% | −0.76 [−1.58, −0.40] |
| English (4, 276 min), remote lines unlabelled | 26.39% | 27.08% | +0.69 [−0.88, +1.74] |
| English, local/remote proxy (mix) | 0.63% | 0.58% | −0.05 [−1.00, +1.38] |

On AMI dev the gain is mostly speaker choice: 7.57% of words go to a wrong speaker against 13.88%, and the unlabelled share barely moves (14.89% against 15.11%). The engine's own turns have 0.37% confusion against 11.24%, and 15.27% missed speech against 5.62%. Nemotron was trained on AMI train and dev, so these figures flatter it. On the held-out test meetings the headline ties, with engine confusion 0.71% against 3.39% and missed speech 20.06% against 9.25%. The product engine's figures match the fn-64 runner's to within 0.1 point on every split, so the product reproduces fn-64's pipeline.

No retained meeting is labelled yet, so the German meetings have proxies and no headline number. Nemotron halves the local/remote confusion on the German mix, and its missed speech leaves more German remote lines outside today's coverage rule.

## Speed

| AMI test, 126 min, cold process, model load included | Wall time | Per hour of audio | Realtime factor | Process CPU time | Peak RSS |
|---|---:|---:|---:|---:|---:|
| Nemotron, CPU (four runtime threads) | 226.6 s | 107.6 s | 33.4x | 839 s | 459 MiB |
| Nemotron, Vulkan (RTX 4090) | 31.5 s | 14.9 s | 240.9x | 30 s | 464 MiB |
| Nemotron, CUDA (RTX 4090) | 24.9 s | 11.8 s | 304.2x | 25 s | 860 MiB |
| Current engine, CPU (fn-64 receipt, four threads) | 557 s | 265 s | 13.6x | 2,208 s | 665 MiB |

The first Vulkan meeting includes compiling the shader pipelines, 9.8 s against 5.0 s for the next, longer meeting. Across the bench's 42 recordings the CUDA build ran at 299x realtime on AMI dev and 307x on the retained meetings. On CUDA and Vulkan the process CPU time equals the wall time, which points at the host-side features and speaker cache as the limit rather than the GPU.

## Conditions and limits

- **Load.** Other workers ran test suites on this machine throughout (load average 15 to 25 on 32 threads). The Nemotron timings above ran at `nice 10` under that load and are an upper bound. The current engine's CPU row is fn-64's, measured at `nice 19` while a meeting recorded.
- **Cached inputs.** The current engine's turns and the Whisper segments on AMI are the bench cache's results from `dettivo-bin` 0.2.x, the build installed before 0.3.0 arrived during this run. No commit touched `dettivo-engine-diarize`, `sherpa-onnx-sys` or `dettivo-engine-whisper` between v0.2.0 and this tree, so they are the same code.
- **The labelling rule.** These figures use the rule on this branch. fn-68 replaces it, and the conductor re-runs the bench after both land. The decision in ADR 0073 follows that re-run.
- **CPU bench column.** A CPU run of the product engine over all 42 recordings was stopped after five, because the load slowed it to about one recording in three minutes. The CPU build's accuracy rests on parity with the CUDA build and the port.
- **The CUDA port.** ONNX Runtime's CUDA provider failed on this host with a cuDNN error, so every parity run used the port on the CPU.

## Reproduce

```sh
python3 scripts/qa/nemotron3/parity_engine.py --engine target/release/dettivo-engine-nemotron \
  --engine-provider cpu --model <models>/diarize/nemotron-3-diarization --onnx model.onnx \
  --ami ami/ES2004a.wav ami/TS3003a.wav ami/IS1009b.wav ami/EN2002c.wav
just diar-bench --full --heldout   # nemotron-cpp is the installed engine; bench.json points it at a build
```

The parity run needs numpy and onnxruntime and reads the AMI references beside each WAV (`<name>.rttm`, `<name>.uem`).
