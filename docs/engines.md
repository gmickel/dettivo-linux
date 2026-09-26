# Engines

Speech recognition, the language model and diarization run in their own processes (ADR 0003): the daemon starts an engine on first need, keeps it warm while it is used, and ends it after an idle timeout so GPU memory comes back. A crash in native inference fails one request, never the daemon, and the engine restarts on the next use. Every engine binary also runs from the command line on a WAV file and prints the same JSON the daemon would get, so benchmarks and fixtures exercise the exact inference path.

## Binaries and where they live

| Binary | Family | Backend |
|---|---|---|
| `dettivo-engine-whisper` | whisper.cpp (ggml) | Vulkan when built with it and a device is installed, CPU otherwise |
| `dettivo-engine-parakeet` | parakeet.cpp (ggml) | Vulkan when built with it and a device is installed, CPU otherwise |
| `dettivo-engine-llm` | llama.cpp (ggml) | Vulkan when built with it, a device is installed and the model fits its free memory, CPU otherwise |
| `dettivo-engine-diarize` | sherpa-onnx (ONNX Runtime) | CPU by default; CUDA through the optional drop-in (ADR 0057) |

The daemon looks for an engine in `[engines] directory` when set, then in `/usr/lib/dettivo/engines-cuda` (the optional CUDA diarization drop-in), then in `/usr/lib/dettivo/engines` (where the package installs them), then beside its own binary, then on `PATH`; `speech.engines` reports the directory that answered ([docs/install.md](install.md)). `dettivo doctor` names each engine it found, the backend of its last load and why, and whether it is degraded.

## Running an engine by hand

```
dettivo-engine-whisper --wav samples/jfk.wav --model ~/.local/share/dettivo/models/whisper/tiny.en/ggml-tiny.en.bin --json
dettivo-engine-whisper --wav clip.wav --model ... --language de --prompt "Dettivo, Omarchy" --cpu --json
```

```
dettivo-engine-parakeet --wav samples/jfk.wav --model ~/.local/share/dettivo/models/parakeet/parakeet-v3/tdt-0.6b-v3-q8_0.gguf --json
dettivo-engine-parakeet --wav clip.wav --model ... --language de --cpu --json
```

```
dettivo-engine-llm --prompt "um so i think we should ship this tomorrow" --system "Rewrite the dictation." --model ~/.local/share/dettivo/models/llm/qwen3-1.7b/Qwen3-1.7B-Q4_K_M.gguf --json
dettivo-engine-llm --prompt "Count to five:" --raw --max-tokens 32 --cpu --model ...
```

```
dettivo-engine-diarize --wav meeting.wav --model ~/.local/share/dettivo/models/diarize/diarization-en --json
dettivo-engine-diarize --wav meeting.wav --model ... --speakers 2 --threads 4 --provider cuda
```

Without `--json` the command prints the transcript text (or the answer, or one line per speaker turn) alone. The JSON is the protocol's `recognize` result: `text`, `language`, `segments` (`start_ms`, `end_ms`, `text`, and for Parakeet `words` with `start_ms`, `end_ms`, `text`, `confidence`), `duration_ms`, `backend`. `--cpu` (or `DETTIVO_FORCE_CPU=1`) forces the CPU backend; a Vulkan build reports `vulkan` when ggml enumerates a device and the model loads there, and falls back to `cpu` with the reason otherwise. `dettivo-engine-llm --backend vulkan` asks for Vulkan or a refusal (`load_failed` naming the preference), as `[engines.llm] backend = "vulkan"` does in the daemon. The reason is on the engine's `model loaded` log line, which `speech.engines` and `dettivo doctor` repeat.

The LLM's JSON is the protocol's `generate` result: `text` (raw, thinking blocks and all; the language crate strips them), `tokens`, `finish_reason` (`stop`, `length`, `cancelled`), `backend`. `--system` and the prompt go through the model's chat template with thinking switched off (the polish profile); `--raw` feeds the prompt as is.

A model replacement unloads the current model before loading the new one, so two individually fitting models do not need simultaneous memory. A failed replacement leaves the host unloaded; it does not restore the old warm model. Parakeet releases its model before resetting the global backend ([ADR 0054](adr/0054-delete-duplicate-work-and-keep-one-owner.md)).

Models live under `$XDG_DATA_HOME/dettivo/models/<provider>/<model-id>/` with a `manifest.json` (ADR 0005); [docs/models.md](models.md) covers the catalogue, downloads and selection. `scripts/models/fetch-test-model.sh` fetches `whisper/tiny.en`, `parakeet/parakeet-v2` and the `jfk.wav` fixture with pinned checksums for the tests, and `scripts/models/fetch-llm-test-model.sh` fetches `llm/qwen3-1.7b` for the model-backed LLM tests (skipped by name without it).

## Parakeet

`dettivo-engine-parakeet` runs NVIDIA's Parakeet TDT 0.6B models through parakeet.cpp (ADR 0018), the ggml port that keeps word error rate parity with the NeMo release and returns a start, an end and a confidence for every word. Pick it with `dettivo speech selection set --provider parakeet`; the daemon switches `[speech] model` to `[speech] parakeet_model_id` when the current model is not a Parakeet one. On a machine without a Vulkan device (the CPU tier, `dettivo doctor` names it) Parakeet is the engine to run: it answers an eleven-second dictation in about 400 ms and transcribes at over twenty times realtime on a desktop CPU where Whisper large-v3-turbo takes seven seconds and runs at about twice realtime, so the CPU tier's targets are stated for it ([ADR 0029](adr/0029-nfr-calibration-and-the-benchmark-suite.md)); set `[speech] provider = "parakeet"` in `config.toml` or run the command above.

| Model | Languages | File | Size |
|---|---|---|---|
| `parakeet-v2` | English | `tdt-0.6b-v2-q8_0.gguf` | 904 MB |
| `parakeet-v3` (default) | Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, Hungarian, Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Slovak, Slovenian, Spanish, Swedish, Russian, Ukrainian | `tdt-0.6b-v3-q8_0.gguf` | 941 MB |

The engine reads the model's name from the GGUF header to know its languages: a `recognize` with a language the model lacks (`de` on v2) is answered with `bad_request` naming the model's languages, and `auto` lets v3 detect the language itself (the result then reports `auto`, since the model does not say which it chose). Parakeet takes no vocabulary prompt; the daemon drops `[dictation] vocabulary` before the engine sees it. Audio over 90 seconds is recognized in 60 second windows overlapping by 2 seconds and stitched at the overlap midpoint by word timestamps; words become segments at pauses over 700 ms and every 30 seconds.

`load` runs half a second of silence through the model so the backend is created on the device `[engines.parakeet] backend` asks for and the weights are on it before the daemon hears `loaded`; the reason names the device parakeet.cpp chose (`Vulkan0`) or why it fell back. The binding, `crates/parakeet-cpp-sys`, fetches parakeet.cpp 0.5.0 and its ggml at build time, verifies their checksums, builds them statically and binds `parakeet_capi.h`; `PARAKEET_CPP_SOURCE_DIR` points it at a local checkout, which the build reads and never writes: the checkout has to carry upstream's ggml CPU patch already (`cd third_party/ggml && patch -p1 < ../ggml-patches/0001-ggml-cpu-*.patch`), and the build stops naming that command when it does not; `PARAKEET_CPP_ARCHIVE_DIR` points at archives already downloaded, and the tree unpacked from them is the one the build patches.

### Word error rate and timestamps

`crates/dettivo-engine-parakeet/fixtures/wer.json` records a ceiling per model and backend on `jfk.wav`; the CLI test runs every row this machine can (v2 on the CPU in CI, v2 and v3 on Vulkan on a development machine with `just test-parakeet-vulkan`) and fails naming the row and the number. Measured: 0.000 for every row.

`dettivo-qa pipeline parakeet-alignment` measures both engines against the golden word alignment ([docs/qa.md](qa.md)); the checked-in run is [docs/reports/parakeet-alignment-report.json](reports/parakeet-alignment-report.json).

## The language model

`dettivo-engine-llm` runs a catalogue GGUF through llama.cpp (the `llama-cpp-2` binding, ADR 0026) for the Enhanced rewrite: the `local` provider of [docs/polish.md](polish.md) sends one `generate` per rewrite with the Polish prompt profile and gets the model's answer back through the same guards every provider goes through. Pick the model with `[llm] model` (`qwen3-4b-instruct-2507` by default; `qwen3-1.7b` is the fast option, `qwen3-4b` the quality one, `qwen3-8b` meeting analysis) and fetch it with `dettivo llm download --model <id>`; [docs/models.md](models.md) lists the four.

`load` takes an optional `lora`, a GGUF LoRA adapter the engine initialises once and applies over the model in every context, which is how a sideloaded fine-tune shipped as an adapter runs over its catalogue base ([docs/polish-models.md](polish-models.md), ADR 0032); the CLI mode takes the same as `--lora <path>`, and `llm.engine.status` reports the adapter in `lora` once loaded. `load` puts every layer on the Vulkan device with the most memory (a discrete GPU before an integrated one) when the build carries the backend, a device enumerates and the file fits its free memory; otherwise the CPU, with the reason in `llm.engine.status` (`the model (N bytes) exceeds the free memory of <device>`, `no Vulkan device enumerates in ggml`, `this build has no Vulkan backend`, or a Vulkan load that failed and fell back). `[engines.llm] backend = "vulkan"` fails the load instead of falling back; `cpu` never touches the GPU. `context_length` sets the context the model is loaded with (4096 by default) and `max_tokens` bounds one answer.

`generate` streams `partial` events as pieces of text land and answers with the whole text, the token count and why it ended; the partials concatenate to the answer, because a piece that could begin a `stop` string is held until it cannot. A `cancel` for the running request stops it between two tokens; the answer so far comes back with `finish_reason = cancelled`. The daemon sends that cancel when a rewrite outlives `[llm] timeout_ms`, so the process is free for the next request instead of finishing an answer nobody waits for; the Polish text is inserted meanwhile with `provider_unavailable`. `status` answers `busy` while a generation runs, and any other request waits its turn, so two rewrites at once (a dictation and a `polish.test`) queue in order, the second inside its own timeout.

`status` also carries `memory_bytes`: the bytes in use on the device the model sits on, as ggml reports them (VRAM under Vulkan). `llm.engine.status` and `dettivo llm engine` show it. After `[engines] llm_idle_seconds` (600 by default) without a request the supervisor sends `unload` and ends the process, and the number drops back to what the desktop was using: on the development machine (RTX 4090, Qwen3 1.7B Q4_K_M, context 2048) the device went from 6,971 MB in use before the load to 8,102 MB with the model loaded and back to 6,933 MB once the reaper had unloaded it, with the rewrite itself taking 650 ms ([docs/reports/llm-idle-unload-report.json](reports/llm-idle-unload-report.json), written by the `DETTIVO_LLM_ENGINE_DIR` run of `crates/dettivo-speech/tests/llm.rs`; [docs/reports/llm-goldens-report.json](reports/llm-goldens-report.json) records the rewrites the same model produced for the Polish goldens).

A context (the KV cache) is created per `generate` and dropped after it, so only the weights stay on the device between requests; the load and the first answer for the 1.7B model take about a second on the CPU of the development machine and less on Vulkan.

## Diarization

NVIDIA users can run the post-meeting speaker pass on CUDA with `cargo build -p dettivo-engine-diarize --features cuda` or by installing the `dettivo-engines-cuda` drop-in ([docs/install.md](install.md)). Both providers use the same downloaded model set. Set `[engines.diarize] backend` to `auto` (the default), `cpu` or `cuda`, through the config file or Settings / Models. The standalone engine takes `--provider auto|cpu|cuda`; `DETTIVO_FORCE_CPU=1` overrides the choice.

`auto` selects CUDA after the provider files, CUDA 13/cuDNN 9 dependencies, ONNX Runtime CUDA entry point and device check pass. A failed prerequisite check selects CPU with the reason. This fallback does not recover from later model initialization failures or GPU out-of-memory errors. `load` reports the actual `backend` and an optional `fallback_reason`, including the missing library. A forced `cuda` load fails with that reason instead of falling back. A missing `libcudart.so.13`, `libcudnn.so.9`, `libonnxruntime_providers_cuda.so` or `libonnxruntime_providers_shared.so` therefore leaves automatic diarization on CPU. The engine process alone loads ONNX Runtime; the daemon and ggml engines remain separate. AMD and Intel keep CPU diarization. [ADR 0057](adr/0057-cuda-diarization-drop-in-with-cpu-fallback.md) records the provider choice and its bounds.

`dettivo-engine-diarize` runs sherpa-onnx's offline speaker diarization (ADR 0035): pyannote segmentation 3.0 cuts the audio into speaker turns and English VoxCeleb ERes2Net embeds each one so the turns cluster into speakers, both from one model directory (`segmentation.onnx`, `embedding.onnx`, the catalogue's `diarize/diarization-en` set of 33.4 MB). `load` names the directory, `provider` (`auto`, `cpu`, `cuda`) and `threads` (0 is the core count capped at four; `[engines.diarize] threads`); `diarize` takes the track as one `pcm16k` attachment with `speakers` as the cluster count when it is known and `clustering_threshold` (0.6) when it is not, sends `progress` events with `completed` and `total` as the embedding chunks finish, and answers `turns` (`start_ms`, `end_ms`, `speaker` as `SPEAKER_00`, `SPEAKER_01`, ... in start order). The pass runs on its own thread inside the engine, so a `status` meanwhile says `busy` and a `cancel` for the running request is answered at once: sherpa-onnx cannot stop mid-way, so the pass finishes in the background, its result is dropped, and the requests that arrived meanwhile are answered afterwards in order. The JSON of the CLI mode is the protocol's `diarize` result; the log line `diarized audio_ms=... elapsed_ms=...` is the benchmark figure. The accuracy report identifies the measured pipeline and model. [A side-by-side evaluation](reports/benchmarks/diarization-nemotron3-2026-09-25.md) measured NVIDIA Nemotron 3 Diarization against this engine on public and retained meetings; the engine stays the shipping default.

The binding, `crates/sherpa-onnx-sys`, builds the C API from sherpa-onnx 1.13.7 source for both CPU and CUDA. Both apply `patches/diarization-calibration.patch` and the same `calibrated_clustering.h`. Automatic mode trains average-linkage cosine clusters at cutoff 0.6 on embeddings with at least two seconds of clean speech. It retains clusters with at least 1% of training embeddings (rounded to even and bounded from one to fifteen), consolidates weak centroid fragments, and assigns every embedding to the retained centroids. Inputs with fewer than two reliable embeddings use all embeddings; a minimum population of one disables consolidation to preserve short-input separation.

Consolidation merges the lowest support-weighted angular cost while `2*d*ni*nj/((ni+nj)*m) <= 1.5`. Here `d` is cosine distance between raw centroids, `ni` and `nj` are their reliable embedding populations, and `m` is the minimum population. The merged raw mean is weighted by those populations. This protects well-supported similar voices while allowing weak fragments to join a speaker. Automatic reconstruction uses float32 vote thresholds of 0.4 for speech and 1.2 for overlapping speech, then applies a 0.1-second duration filter and 0.1-second gap filling. A known speaker count retains upstream complete linkage, its original vote rule, and 0.3/0.5-second filtering and gap filling. [ADR 0058](adr/0058-strict-diarization-accuracy-evaluation.md) records calibration and validation status.

The source archive SHA-256 is `ee0c20cafb34cc1f86afb2845babd941c26e46de4a9925cbe86fd55ff3557818`. `source_build.rs` fetches five individually checksummed ONNX Runtime v1.27.1 headers and uses the verified release's ONNX Runtime binaries. CUDA additionally applies `patches/cuda-conv-default.patch`, changing convolution search from `Heuristic` to `Default`, and retains both provider libraries. Patches apply with zero fuzz; their hashes and the clustering header hash identify the source cache. Both builds need CMake, Ninja, a C++ compiler and GNU patch. `CMAKE_BUILD_PARALLEL_LEVEL` overrides the default eight build jobs.

`SHERPA_ONNX_ARCHIVE_DIR` supplies cached release and source archives; `SHERPA_ONNX_DIST_DIR` supplies the unpacked runtime release while both providers still build the patched C API. Standalone builds retain the run paths `$ORIGIN`, `$ORIGIN/../lib/dettivo` and the build's own library directory. Packaging sets `DETTIVO_PACKAGE=1` and emits only `$ORIGIN`, with no build paths. The CPU package places two libraries beside the engine; the CUDA package places four there, including both provider libraries. ONNX Runtime is loaded into this process alone. `docs/meetings.md` describes what the daemon does with the turns; `dettivo-qa pipeline diarization` scores the engine against the two-speaker fixture ([docs/qa.md](qa.md)).

## Capability matrix

| Provider | Timestamps | Word timestamps | Vocabulary prompt | Dictation | Meetings (`meeting_capable`) |
|---|---|---|---|---|---|
| `whisper` | segments | no | yes | yes | yes |
| `parakeet` | segments and words with confidence | yes | no | yes | no: the spike measured a pooled start/end p95 of 308 ms (v2) and 347 ms (v3) against the merger's 200 ms; ADR 0018 records the route to change this |

`system.capabilities.speech.providers.<id>.meeting_capable` and `speech.providers.list[].supports_meetings` carry the last column, so a client offers a provider for meetings by the flag, never by its name.

## The protocol

One framed stream on the engine's stdin and stdout (stderr carries logs): a big-endian `u32` header length, a JSON header, then any attachments' bytes in order.

```
{ "v": 1, "id": 3, "kind": "request", "name": "recognize",
  "payload": { "language": "en", "prompt": "Dettivo", "timestamps": true },
  "attachments": [ { "kind": "pcm16k", "bytes": 32000 } ] }
```

Requests: `load` (`model`, `backend_preference` = `auto` | `vulkan` | `cpu`, optional `vad_model`, optional `context_length` and optional `lora` for the language model, optional `threads` and `provider` = `auto` | `cpu` | `cuda` for the diarization engine), `unload`, `recognize` (one `pcm16k` attachment: 16 kHz mono signed 16-bit little-endian), `generate` (`prompt_profile` = `polish` | `raw`, `system`, `user`, `max_tokens`, `temperature`, `stop`), `diarize` (one `pcm16k` attachment; `speakers`, `clustering_threshold`), `cancel` (`request_id`), `status` (`loaded`, `model`, `backend`, `busy`, and `memory_bytes` when the backend reports it). Responses echo the request id and carry the typed result (`generate` answers `text`, `tokens`, `finish_reason`, `backend`; `diarize` answers `turns`), or `error` with a stable `code` (`model_missing`, `load_failed`, `bad_request`, `cancelled`, `internal`) and a message that never contains transcript text, a prompt or audio. Events: `progress` (`request_id`, `fraction`, and `completed` with `total` on a pass that counts chunks), `partial` (`request_id`, `text`), `loaded` (`model`, `backend`, `reason`), `error`.

The speech engines answer one request at a time and a `cancel` takes effect on the next request; the language model engine keeps reading while it generates, so a `cancel` for the running request lands between two tokens and a `status` meanwhile says `busy`; the diarization engine keeps reading while a pass runs, answers a `cancel` at once and drops the pass's result when it ends.

The shapes are fixture-tested under `crates/dettivo-engine-proto/fixtures/`, one file per message, the way the IPC contract is.

The engine frame reader and writer share a 460,800,000-byte aggregate attachment cap. A four-hour 16 kHz mono signed 16-bit track fits exactly. Diarization validates duration before reading or encoding PCM; a larger configured import may transcribe but its diarization refuses more than 14,400 seconds. The full track remains an in-memory input, about 439.5 MiB at the maximum (ADR 0055).

## The supervisor

`dettivo-speech` holds the `SttEngine` trait (the macOS capability contract: timestamps, streaming, languages, maximum duration, custom vocabulary, model deletion, plus `preload`, `recognize`, `cancel`) and the supervisor behind it:

- spawn on first need, `load` the selected model, keep the process warm;
- after `[engines] stt_idle_seconds` without a request (`llm_idle_seconds` for the language model engine; the diarization engine idles out with the speech engines), `unload` and end the process; an engine that does not answer the `unload` is ended all the same and the log says so;
- after a crash, restart on the next request with backoff (1, 2, 4, 8 seconds); three crashes in a row mark the engine degraded until a load succeeds, and `dettivo doctor` says so;
- preloads from `startup`, `model_selection` and `session_start` coalesce into one load, never download, and fail without failing the caller;
- a session freezes its engine and model at start.

Status and capability-tier requests use a published snapshot while inference holds a slot, so they return without waiting for the job. A busy slot's resident-memory figure is its last observation. Idle reaping skips an occupied slot and checks it on the next pass (ADR 0055).

## Watching engines

`dettivo speech engines` (the `speech.engines` method, a Linux addition) lists every engine binary the daemon knows: where it was found, whether it runs, the loaded model, the backend of the last load and why, the crash count and the degraded flag. `dettivo doctor` prints the same rows and exits 1 while an engine is degraded:

```
engine    dettivo-engine-whisper running /home/you/.local/share/dettivo/models/whisper/large-v3-turbo/ggml-large-v3-turbo.bin on vulkan
engine    dettivo-engine-parakeet found, not running
engine    dettivo-engine-llm found, not running
engine    dettivo-engine-diarize found, not running
llm       provider local; local available (llm/qwen3-4b-instruct-2507 ready); dettivo-engine-llm found, not running
diarize   model diarize/diarization ready; dettivo-engine-diarize found, not running
```

`dettivo llm engine` (the `llm.engine.status` method) adds what the language model process reports: whether a model is loaded, whether it is busy, the bytes it holds on its device and the idle after which the daemon unloads it. Every `speech.engines` row carries `memory_bytes`, the process's resident set while it runs, and `system.capabilities.platform.tier` says which hardware tier the machine benchmarks as (`gpu` when a Vulkan device is installed and no loaded engine with a GPU path fell back to the CPU, `cpu` otherwise or under `DETTIVO_FORCE_CPU=1`; the optional diarization provider does not change this ggml-based tier) with the reason; `dettivo doctor` prints both ([docs/qa.md](qa.md) has the benchmark suite that reads the tier).

The daemon preloads the `[speech]` model at start and after a change to `[speech] model`, when the model file exists; `[engines] stt_idle_seconds` and `directory` take effect at the next reload without a restart.

## Building with Vulkan

`just build` compiles the three ggml engines for the CPU. `just build-whisper-vulkan`, `just build-parakeet-vulkan` and `just build-llm-vulkan` add the ggml Vulkan backend (`--features vulkan`) and need `vulkan-headers`, `spirv-headers`, `vulkan-icd-loader` and `shaderc`; CI builds them in the advisory `whisper-vulkan`, `parakeet-vulkan` and `llm-vulkan` jobs (the current whisper.cpp shader generation does not link against the container's shaderc yet, and a runner has no device to run on, so the jobs report without blocking). A Vulkan build under `backend = "auto"` still falls back to the CPU, with the reason in `speech.engines` or `llm.engine.status`, when no device enumerates in ggml (an installed ICD alone is not a device); under `backend = "vulkan"` the load fails naming the preference instead, and an engine never reports `vulkan` for a model it opened on the CPU. `[engines.whisper] backend`, `[engines.parakeet] backend` and `[engines.llm] backend` pin `vulkan` or `cpu` per engine ([docs/config.md](config.md)). `just test-llm-vulkan` runs the model-backed LLM tests on Vulkan on a development machine (`DETTIVO_TEST_BACKEND=vulkan` makes every model-backed route load with the strict preference and assert `backend = vulkan`, so a CPU fallback fails the suite), and `DETTIVO_LLM_ENGINE_DIR=<dir with the Vulkan dettivo-engine-llm> cargo test -p dettivo-speech --test llm` runs the idle-unload test that writes the memory report; without the system headers, `CMAKE_PREFIX_PATH` pointing at an unpacked Vulkan-Headers and SPIRV-Headers install works for the build.

## Logs

Engines log to stderr, which the supervisor keeps (the last twenty lines, bounded) for a crash report. No log line from the engine or the supervisor carries transcript text, prompts or audio; the engine crate's tests drive marker strings through the CLI and the protocol and fail if one reaches the log.
