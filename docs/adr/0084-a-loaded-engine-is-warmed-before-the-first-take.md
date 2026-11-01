# 0084. A loaded speech engine is warmed before the first take, idles out after 300 s, and runs four CPU threads on the GPU

Status: Accepted 2026-11-01.

## What this gives you

The first dictation after the daemon starts, or after the engine idled out, no longer pays for the work a fresh engine does on its first inference, once the graphics driver has compiled the engine's shaders on this machine. `dettivo speech status` and `dettivo doctor` say whether the next take finds the engine cold, loaded or warm. The idle timeout and the GPU thread count rest on the measurements in [docs/reports/benchmarks/2026-11-01-thor-gpu.json](../reports/benchmarks/2026-11-01-thor-gpu.json).

## Situation

The daemon preloaded the selected model at start, after a model change and when a session started, but ran no inference. The first `recognize` on a loaded engine then did work the later ones skip. The benchmark's `warm_up` step measured it on the reference machine (Ryzen 9 7950X3D, RTX 4090, Vulkan, the debug engine builds `just bench` uses), with the driver's shader cache already holding the engines' pipelines:

| Engine | Load (process start to `load` answer) | First take on the loaded engine | First take after the warm-up | Warm take p50 | Idle GPU memory |
|---|---|---|---|---|---|
| Whisper large-v3-turbo | 307 ms | 161 ms | 148 ms | 137 ms | 865 MiB |
| Parakeet v3 | 581 ms | 71 ms | 53 ms | 52 ms | 954 MiB |

A take on an engine that is not running costs the load plus the first take, 468 ms for Whisper and 652 ms for Parakeet. An earlier quick run measured Whisper's first take on a loaded engine at 611 ms against 133 ms warm.

The graphics driver's shader cache costs more than the engine. With an empty cache (a fresh profile, a driver update, the first run on a machine) the NVIDIA driver compiles every Vulkan pipeline on first use: 8.7 s for Whisper's first inference and 4.0 s for Parakeet's. The driver keeps them on disk, and later engine processes skip the compile.

The warm-up's length decides how much of that it covers. On an empty cache, 1.5 s of silence left Parakeet's first 11 s take at 2.7 s, 10 s left it at 1.4 s, and 30 s brought it to 88 ms against 54 ms warm. Whisper decodes a 30 s window whatever the input length, so its warm-up costs the same at every length. Silence, noise and a vocabulary prompt all left Whisper's first take on an empty cache at 425 to 585 ms against about 150 ms warm. None of those decodes any text, and the text decoder's pipelines compile on the first take that produces words.

`[engines] stt_idle_seconds` was 300. The candidates were 300 s, 1800 s and never unloading while the session is graphical.

On a GPU backend only the decoder and the glue run on the CPU, and the speech engines used the core count capped at eight threads. The `gpu_threads` step loaded each engine with 1, 2, 4, 8 and 16 threads and timed ten warm takes each. Whisper's p50 was 160, 148, 140, 138 and 139 ms. Parakeet's was 55, 55, 55, 56 and 55 ms.

## Decision

- After every successful preload, the supervisor runs one `recognize` of 30 s of zeros against the loaded model and discards the result. Only its time reaches the log (`engine warmed up warm_up_ms=...`). It produces no history item, no event with text and no log line with text.
- The supervisor skips the warm-up when the engine recognised within the last 60 s, when the model is already warm, and when a real request has asked for the engine. A take that asks first runs first and warms the engine itself. A take that asks while a warm-up runs waits for that one warm-up and nothing more.
- A loaded model is warm once it has run the warm-up or a take. `speech.engines` carries `warm`, `engine.state` announces the step with the state `warm`, and the CLI reads a row as cold (no model loaded), loaded (not run yet) or warm.
- `[engines] stt_idle_seconds` stays 300. The session-start preload spawns, loads and warms the engine while the person speaks, so a take longer than the load and the warm-up (about 0.5 to 0.7 s here with a filled shader cache) waits for none of it. An idle engine holds 865 to 954 MiB of GPU memory that a game or the local language model could use, and 1800 s or never would hold it through every longer pause.
- A speech engine on a GPU backend runs its CPU work on four threads (`dettivo_engine_proto::backend::GPU_THREADS`) unless the `load` names a count. Four is 2 ms (1.5%) from the fastest count for Whisper, and Parakeet's times did not move with the count. The CPU backend keeps the core count capped at eight.
- The benchmark records the first take after daemon start against the warm p50 as `first_insert.cold_ratio` and `cold_met` (the 20% bound), and the `warm_up` and `gpu_threads` steps record the figures above on every `just bench`.

## Consequences

- With the driver's shader cache filled, which is every daemon start after the first on a machine, the first take after the warm-up costs what a warm take costs: 148 ms against 137 ms for Whisper (ratio 1.08) and 53 ms against 52 ms for Parakeet (1.02).
- The first run after an install or a driver update misses the 20% bound. The `first_insert` step runs its daemons in fresh profiles, so it measures exactly that case. The warm-up compiled the pipelines during the first take's 10 s capture, but took 10.1 s for Whisper and, after a 3.5 s load, 7.2 s for Parakeet, so the first take waited for the rest: 1,066 ms against a warm p50 of 522 ms for Whisper (ratio 2.04) and 795 ms against 266 ms for Parakeet (2.99). Whisper's text decoder also compiles on the first take that produces words, about 0.4 s once per shader cache.
- The warm-up costs one inference over 30 s of silence after each load, 173 ms for Whisper and 118 ms for Parakeet with a filled cache.
- A model change within 60 s of a take loads without a warm-up and reads loaded, so the next take pays the first-inference cost once.
