# Evaluate Nemotron 3 Diarization against current speaker pass

Stub. Experiment only. Do not replace the current speaker pass until this measures a real win on Dettivo Linux meetings.

## Goal & Context

Jan Kees van Weert shipped Meeting Recorder 1.1 with NVIDIA Nemotron 3 Diarization. On two real meetings the wrong-speaker rate went from 17% and 54% to 2% and 6%, in about a quarter of the time, with overlap handling up to eight speakers.

We need the same comparison on Dettivo Linux's current diarize engine (Sherpa-ONNX / ggml path, CPU default, optional CUDA from fn-42), not a tweet-shaped rewrite. If Nemotron is actually that much better here, a later spec can swap or add it. This spec only answers whether it is.

Source: https://x.com/jankeesvw/status/2103235751739793688
Release: https://github.com/jankeesvw/omarchy-meeting-recorder/releases/tag/v1.1.0

## Approach

- Keep the current diarize engine as the baseline.
- Run Nemotron 3 Diarization as a side-by-side post-pass on the same retained meeting audio.
- Score wrong-speaker rate and wall time on at least two real Dettivo meetings (mic + system tracks where we have them).
- Report numbers. No product default change in this spec.

## Quick commands

- `just build test lint`

## Acceptance

- **R1:** Baseline wrong-speaker rate is measured with the current diarize engine on at least two retained real meetings, with the scoring method written down.
- **R2:** The same meetings are scored with Nemotron 3 Diarization on this machine, including wall time versus baseline.
- **R3:** A short evidence note records both rates, both times, model/runtime identity (CPU vs CUDA), and whether overlap/crosstalk improved.
- **R4:** Shipping default stays the current diarize engine unless a follow-up spec, with these numbers, says otherwise.
- **R5:** [user] At least one German retained meeting is included, and German results are reported separately from English alongside the pooled numbers.
- **R6:** [user] Nemotron is also scored with segment-based speaker assignment: each Whisper transcript segment takes the speaker with the highest mean probability over the segment, with no 0.5 threshold. The report compares it with the recorder post-processing and the current engine on lines left without a speaker, on the judged wrong-speaker rate for English, German and pooled, and on AMI confusion and missed speech. The change stays inside a Nemotron-specific adapter.

## Boundaries

- No replacement of `dettivo-engine-diarize` in the daemon or package.
- No AUR/release, no Settings/config default change.
- No confidential transcript text in the spec or git; keep audio and transcripts in protected local storage.
- macOS comparison is a sibling spec on `dettivo` (`fn-83-evaluate-nemotron-3-diarization-against`), not this checkout.

## Decision Context

Triggered by the Meeting Recorder 1.1 claim. Linux already has a speaker pass and a CUDA drop-in; the only question is whether Nemotron 3 beats that enough on our meetings to justify a later integration spec.

## Resolved via Research
<!-- provenance: refine --scope=research (docs-scout, practice-scout, docs-gap-scout, memory-scout) on 2026-09-25; plan writes the same section when its Step 1 runs the same scouts -->

### docs-scout
- **nvidia/Nemotron-3-Diarization (rev f667ed7, 2026-09-24)**: a single-pass Sortformer-family model, with no embedding or clustering step. One checkpoint runs streaming or offline, handles up to 8 speakers, takes 16 kHz mono, and returns per-speaker activity probabilities at 10 ms frames. Long audio goes through chunked inference. Source: https://huggingface.co/nvidia/Nemotron-3-Diarization
- **Licence `openmdw-1.1`** on both the NVIDIA and onnx-community repos, confirmed from the HF API card metadata. Read the commercial-use terms before any follow-up integration spec relies on it. Source: https://huggingface.co/api/models/nvidia/Nemotron-3-Diarization
- **Runtimes available**: NVIDIA ships `.nemo`, `model.safetensors` and a `q8_0.gguf`. onnx-community ships fp32, fp16, int8, q4 and q4f16 ONNX graphs, reported within 0.02 to 0.03 DER points of the PyTorch reference. sherpa-onnx does not wrap this model. Source: https://huggingface.co/onnx-community/Nemotron-3-Diarization-ONNX
- **omarchy-meeting-recorder v1.1**: Rust. It drops sherpa-onnx (pyannote plus WeSpeaker) for `ort = "2.0.0-rc.13"` driving the ONNX model on CPU. Post-processing is a 0.5 probability threshold, same-speaker gaps under 500 ms bridged, and blips under 300 ms dropped. Its "wrong-speaker" metric is not named. Source: https://github.com/jankeesvw/omarchy-meeting-recorder/pull/1/files and Cargo.toml at tag v1.1.0
- **Baseline identity**: sherpa-onnx offline diarization with pyannote segmentation 3.0 and English ERes2Net embeddings, with CPU/CUDA chosen by dlopen probing. This is the same stack family the recorder replaced, so the comparison is like-for-like. Source: crates/dettivo-engine-diarize/src/engine.rs:1-23, crates/dettivo-engine-diarize/src/provider.rs
- **Metric mapping**: "wrong-speaker rate" most plausibly means the speaker-confusion component of DER. DER is false alarm plus missed speech plus confusion. Source: https://www.pyannote.ai/blog/how-to-evaluate-speaker-diarization-performance

### practice-scout
- **Gotcha, reference bias:** a reference built by correcting one system's output favours that system. Seed the reference from neither system, or correct against both. Source: https://arxiv.org/pdf/2007.01216
- **Gotcha, collar semantics differ by tool:** a pyannote collar of 2X equals a dscore or md-eval collar of X. Report collar 0 with overlap included (the ADR 0058 convention) and state it. Source: https://www.pyannote.ai/blog/how-to-evaluate-speaker-diarization-performance
- **Gotcha, word-level view:** DER can understate swaps that users notice. WDER (cpWER minus WER) isolates word-level wrong-speaker cost, so report it only if transcripts align cheaply. Source: https://www.emergentmind.com/topics/word-diarization-error-rate-wder
- **Gotcha, timing fairness:** report model load separately from steady-state RTF, pin the thread count, and never blend CPU and CUDA into one number. Source: https://www.emergentmind.com/topics/inverse-real-time-factor-rtfx
- **Gotcha, NeMo environment:** install torch first and add NeMo on top. `uv sync --locked` can replace a pinned torch/CUDA. Torch 2.6 and later may need `weights_only=False` for .nemo checkpoints. The ONNX or GGUF route avoids this entirely. Source: https://github.com/NVIDIA-NeMo/Speech/discussions/15421
- **Gotcha, two tracks:** the mic track picks up bleed from system audio. Score per track and on the mix separately rather than trusting only a mixed-down run. Source: https://github.com/nfishel48/Jotter/issues/12

### docs-gap-scout
- **Docs that must change:** a new `docs/reports/benchmarks/diarization-nemotron3-real-meetings-<date>.{md,json}` report, mirroring `diarization-real-meetings-2026-09-09.{md,json}`. It carries aggregate numbers and identities only, and stays out of the `just bench` README table. Source: docs/reports/benchmarks/diarization-real-meetings-2026-09-09.md:1
- **Reuse the scorer:** `scripts/qa/diarization_score.py` is the strict DER scorer (zero collar, overlap included, optimal mapping, native JSON turns). R1's "scoring method" means this scorer plus its confusion component. Source: scripts/qa/diarization_score.py:1-10, docs/adr/0058-strict-diarization-accuracy-evaluation.md:1
- **Baseline invocation:** `dettivo-engine-diarize --wav --model --json --speakers --threads --provider`, as in the `qa-diarization` recipe. Source: justfile:312-319, docs/engines.md:34-35
- **ADR linkage:** no new ADR, because R4 changes no decision. ADRs 0035 and 0058 gain a pointer only if a follow-up spec acts on the numbers. Source: docs/adr/0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md:1
- **Input audio:** retained meetings live under `$XDG_DATA_HOME/dettivo/meetings/<id>/`, with takes JSON and raw WAVs for mic and system. This machine had 13 meetings and 28 WAVs retained on 2026-09-25. Source: docs/meetings.md:97 and a local listing

### memory-scout
- No applicable entries. The three existing entries are about UI and QA.
