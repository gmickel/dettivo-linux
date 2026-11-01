# 0084. A loaded speech engine is warmed before the first take, idles out after 300 s, and runs four CPU threads on the GPU

Status: Accepted 2026-11-01.

## What this gives you

The first dictation after the daemon starts, or after the engine idled out, no longer pays for the work a fresh engine does on its first inference. On the reference machine it took 330 ms against 296 ms for a warm Whisper take and 149 ms against 185 ms for Parakeet. That holds on every daemon start once the graphics driver has built its shader cache. The first Whisper dictation after an install or a GPU driver update is the one accepted exception: it took 783 ms against 306 ms warm, once per shader cache (see the Decision). `dettivo speech status` and `dettivo doctor` say whether the next take finds the engine cold, loaded or warm. The idle timeout and the GPU thread count rest on the measurements in [docs/reports/benchmarks/2026-11-01-thor-gpu.json](../reports/benchmarks/2026-11-01-thor-gpu.json).

## Situation

The daemon preloaded the selected model at start, after a model change and when a session started, but ran no inference. The first `recognize` on a loaded engine then did work the later ones skip. The benchmark's `warm_up` step measured it on the reference machine (Ryzen 9 7950X3D, RTX 4090, Vulkan, the debug engine builds `just bench` uses, the GPU shared with other work during the run):

| Engine | Load (process start to `load` answer) | First take on the loaded engine | First take after the warm-up | Warm take p50 | Idle GPU memory |
|---|---|---|---|---|---|
| Whisper large-v3-turbo | 366 ms | 208 ms | 184 ms | 178 ms | 865 MiB |
| Parakeet v3 | 664 ms | 93 ms | 69 ms | 68 ms | 954 MiB |

A take on an engine that is not running costs the load plus the first take, 574 ms for Whisper and 757 ms for Parakeet. A quieter run earlier the same day measured Whisper at 161 ms for the first take against 137 ms warm.

The graphics driver's shader cache costs more than the engine. With an empty cache (a fresh profile, a driver update, the first run on a machine) the NVIDIA driver compiles every Vulkan pipeline on first use: 8.7 s for Whisper's first inference and 4.0 s for Parakeet's. The driver keeps them on disk, and later engine processes skip the compile.

The warm-up's length decides how much of that it covers. On an empty shader cache, 1.5 s of silence left Parakeet's first 11 s take at 2.7 s, 10 s left it at 1.4 s, and 30 s brought it to 88 ms against 54 ms warm. Whisper decodes a 30 s window whatever the input length, so its warm-up costs the same at every length. Silence, noise and a vocabulary prompt all left Whisper's first take on an empty cache at 425 to 585 ms against about 150 ms warm. None of those decodes any text, and the text decoder's pipelines compile on the first take that produces words.

`[engines] stt_idle_seconds` was 300. The candidates were 300 s, 1800 s and never unloading while the session is graphical.

On a GPU backend only the decoder and the glue run on the CPU, and the speech engines used the core count capped at eight threads. The `gpu_threads` step loaded each engine with 1, 2, 4, 8 and 16 threads and timed ten warm takes each. In the recorded run Whisper's p50 was 197, 184, 178, 175 and 174 ms and Parakeet's 68, 69, 68, 69 and 68 ms. Two other runs that day agree (Whisper 158, 144, 140, 139 and 137 ms in the quieter one). A third, run while another job loaded the GPU, measured Whisper 16 threads at 149 ms against 176 ms for eight, and its warm takes all ran 40 ms slower than in the other runs.

## Decision

- After every successful preload, the supervisor runs one `recognize` of 30 s of zeros against the loaded model and discards the text. Only its time reaches the log (`engine warmed up warm_up_ms=...`), and the backend it answers updates the engine's record when the engine moved to the CPU. It produces no history item, no event with text and no log line with text.
- The supervisor skips the warm-up when the engine recognised within the last 60 s, when the model is already warm, and when a real request has asked for the engine. A take that asks first runs first and warms the engine itself. A take that asks while a warm-up runs waits for that one warm-up and nothing more.
- A loaded model is warm once it has run the warm-up or a take of that same model. `speech.engines` carries `warm`, `engine.state` announces the step with the state `warm`, and the CLI reads a row as cold (no model loaded), loaded (not run yet) or warm.
- `[engines] stt_idle_seconds` stays 300. The session-start preload spawns, loads and warms the engine while the person speaks, so a take longer than the load and the warm-up (0.6 to 0.8 s here) waits for none of it. An idle engine holds 865 to 954 MiB of GPU memory that a game or the local language model could use, and 1800 s or never would hold it through every longer pause.
- A speech engine on a GPU backend runs its CPU work on four threads (`dettivo_engine_proto::backend::GPU_THREADS`) unless the `load` names a count. Four is within 4 ms (2.3%) of the fastest count for Whisper in the recorded run, and Parakeet's times did not move with the count. The CPU backend keeps the core count capped at eight.
- R2 (the first dictation after daemon start within 20% of a warm one) holds with the driver's shader cache built. Gordon accepted the empty-cache case on 2026-11-01 as an exception to R2: the first take after an install or a GPU driver update, when the driver has no compiled pipelines on disk. A bench with an empty shared cache measured Whisper's first take at 783 ms against a warm p50 of 306 ms (ratio 2.56) and Parakeet's at 197 ms against 177 ms (1.11, within the bound). Whisper misses because its text decoder's pipelines compile only on a take that produces words, and the warm-up is silent by design, so it never reaches them. Warming Whisper with a speech clip instead would close the gap at the cost of a bundled clip and a warm-up that is no longer silent, and the gap appears once per shader cache while every later daemon start meets R2.
- The benchmark's `first_insert` step measures R2 the way a user's daemon meets it. Its daemons share one driver shader cache (`target/bench-shader-cache`), as a user's daemon starts share the driver's own, and the first take starts once the start-up preload reports the engine warm. The step records the wait (`warm_wait_ms`), the first take against the warm p50 (`cold_ratio`) and whether it is within 20% (`cold_met`). The `warm_up` and `gpu_threads` steps record the figures above on every `just bench`.

## Consequences

- In the recorded run the first take came 2.2 s after the daemon answered its ping for Whisper and 2.4 s for Parakeet. It took 330 ms against a warm p50 of 296 ms for Whisper (ratio 1.11) and 149 ms against 185 ms for Parakeet (0.81). The `warm_up` step's first take after the warm-up was 184 ms against 178 ms for Whisper (1.03) and 69 ms against 68 ms for Parakeet (1.01).
- The first Whisper dictation after an install or a driver update takes about 480 ms longer than a warm one (783 ms against 306 ms), the accepted exception to R2 above. The warm-up compiles everything but Whisper's text decoder, which compiles on that first take that produces words and then stays in the driver's cache. A first take that starts before the warm-up ends also waits for the rest of it: 10.1 s for Whisper on an empty cache.
- The bench fills `target/bench-shader-cache` on its first run on a machine, so that run's `first_insert` measures the first-install case and later runs the case every later daemon start meets.
- The warm-up costs one inference over 30 s of silence after each load, 204 ms for Whisper and 142 ms for Parakeet.
- A model change within 60 s of a take loads without a warm-up and reads loaded, so the next take pays the first-inference cost once.
