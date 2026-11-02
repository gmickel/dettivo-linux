# 0098. Parakeet warms up on its longest single pass, 90 s, so no take compiles its own GPU kernels

Status: Accepted 2026-11-02. Amends 0084.

## What this gives you

After a GPU driver update or a new Dettivo build, the first Parakeet dictation past 40 s no longer waits 2.5 to 2.9 s for the driver to compile kernels. That wait is what the four History stalls ADR 0094 left without a cause fit. The warm-up after each Parakeet load now covers every input length the engine runs in one pass. On a machine whose shader cache is current, that costs 180 ms more GPU time per load, behind the take you are speaking. The first take after the engine idled out still waits for nothing on takes of 3 s or longer. Through the daemon, five such 3 s takes took 266-267 ms against a median of 386 ms for warm takes ([the benchmark report](../reports/benchmarks/release-to-text-2026-11-02.md)).

## Situation

ADR 0094 counted seven stalls of 1.1-3.9 s among 93 recorded Parakeet takes. Three were the first take after the engine idled out, and four came 25-107 s after the take before them with no recorded cause. fn-90 asked where that time goes.

The engine alone, timed over the frame protocol (`dettivo-engine-parakeet` release build with Vulkan, Parakeet Ultra, the jfk fixture repeated to length, RTX 4090 shared with other agents), answers it. A new process loads the model, runs the warm-up, and then recognises takes of rising length. The driver's shader cache under `$XDG_CACHE_HOME/nvidia` starts empty, as it does after a driver update or for a build whose shaders changed.

| Input after the load (ms) | 30 s warm-up | 90 s warm-up | Shader cache already filled |
| --- | --- | --- | --- |
| Load | 16,065 | 5,556 | 651-728 |
| Warm-up | 8,518 | 15,436 | 75-78 (30 s), 259-267 (90 s) |
| 3 s / 20 s / 30 s take | 27 / 82 / 105 | 30 / 77 / 99 | 22 / - / 99-108 |
| 40 s take | 144 | 129 | - |
| **45 s take** | **2,867** | 133 | - |
| 60 s / 75 s take | 195 / 260 | 180 / 210 | - |
| **90 s take** | **625** | 265 | - |
| 100 s / 120 s take | 375 / 390 | 270 / 355 | - |

An earlier run with the same 30 s warm-up showed the same two steps: 2,476 ms at 45 s and 609 ms at 90 s, against 324 ms at 60 s.

The NVIDIA driver compiles each Vulkan pipeline on its first use and keeps it on disk. ggml's Parakeet graph uses different pipelines as the input grows, so a 30 s warm-up compiles only those of inputs up to about 40 s. The first longer take compiles the rest while you wait. Once the cache holds them, later engine processes pay nothing: a second process on the same cache loaded in 1,257 ms and ran every length from 5 s to 120 s at its warm time.

The History store fits this. It was read for timings only, never text. The daemon that recorded it had no warm-up (ADR 0084 came later), so a first take after the engine idled out carried the load and the first inference. Its stalls fall on the first takes after a change that replaces the cached kernels, and on the first take of a longer length after that.

- 2026-09-23, the morning after the hotfix engine builds of 2026-09-22 (their profiles under `~/.local/state/dettivo`): a 7.3 s take took 3,130 ms after an overnight idle, and the next 18.3 s take 2,027 ms.
- 2026-10-13: a 0.1 s take took 1,084 ms after an overnight idle. The 49.4 s take two minutes later was the first take past 40 s since 0.4.3 was installed on 2026-10-01 and took 2,765 ms. A 142.9 s take six minutes after it took 715 ms.
- 2026-10-25 and 26, NVIDIA driver 610.57.04 to 615.78.08 (`/var/log/pacman.log`): on 2026-10-27 a 3.7 s take took 2,955 ms after an idle and the next 20.8 s take 1,706 ms. On 2026-10-30 the first take past 40 s since the update, 69.6 s long, took 3,862 ms.

So the four stalls ADR 0094 could not explain are the first take of a longer length after a driver update or a new build, and the three after an idle add a load and a first compile. The store cannot show whether the driver cache was current before each idle take. That part is an inference from the dates, not a measurement.

The options measured:

- **Keep the 30 s warm-up.** On a current cache it costs 77 ms, but after every driver or engine update the first take past 40 s and the first past 90 s each wait for their kernels: 2.5-2.9 s and about 0.3 s.
- **Warm up on 90 s of silence,** `align::SINGLE_PASS_SAMPLES`, the longest input Parakeet runs in one pass (a longer take runs as 60 s windows). Every later length ran at its warm time. On a current cache the warm-up costs 259-267 ms instead of 75-78 ms. On an empty cache it costs 15.4 s instead of 8.5-10.6 s, once per update.
- **Keep the engine loaded longer** (`stt_idle_seconds`). A reload on a current cache takes 651-728 ms, and the preload at the press already hides it for takes of 3 s or longer. A longer idle time holds 954 MiB of GPU memory for longer and does nothing for a cache an update emptied.
- **Preload on the press of the hold key.** The daemon already preloads at every session start (`session_start`, ADR 0084), and `dettivo dictation press` starts the session.

## Decision

Parakeet's warm-up recognises 90 s of silence (`dettivo_engine_proto::warm_up::PARAKEET_SAMPLES`, 1,440,000 samples). Whisper keeps 30 s, the one window it decodes. The warm-up still runs only on a GPU backend, so fn-80's rule that a CPU load needs no warm-up holds. It still yields to a take that asked first, and it is still skipped within 60 s of a recognition by the running engine process (ADR 0084).

Ruled out: a warm-up per length class (three passes cost more than one 90 s pass and need the class edges kept in step with ggml); a longer default `stt_idle_seconds`.

## Consequences

- On a current cache, the stall of the first take after an idle unload no longer reproduces, with either warm-up. In the daemon (`stt_idle_seconds = 61`, so the warm-up runs on each reload), the preload at the press loaded the model in 573-682 ms and finished the 90 s warm-up 663-927 ms into the take. The first 3 s take after each of five unloads took 266-267 ms against 358-481 ms for the takes after it. With the 30 s warm-up it was 275-279 ms against 314-476 ms. fn-80's preload at the press had already removed the stall that History recorded for the first take after idle. What remains is the compile after a cache reset, and this record moves that compile into the warm-up.
- A take that ends while the 90 s warm-up runs waits for the rest of it: up to about 0.27 s on a current cache, and up to 15 s on the first load after an update, when that take would otherwise have compiled its own kernels anyway.
- The bench tool's warm-up (`crates/dettivo-qa/src/bench/engine_client.rs`) sends each engine the same silence the supervisor does, read from `dettivo_engine_proto::warm_up`, so the bench times the warm-up the daemon runs.
- For Parakeet the silent warm-up is enough: after it, on an empty cache, the first takes of real speech ran at their warm time (3 s in 30 ms, 20 s in 77 ms). Whisper differs: its text decoder's kernels compile only on a decode that produces words, so its first take on an empty cache stays the exception ADR 0084 accepts, about 1.2 s (fn-89's measurement). This record leaves Whisper's warm-up as it was.
- The first load after a driver update or a new build still compiles every kernel once: 15 s of warm-up on the reference GPU. A take that ends during it waits, as the first take after an update always did. Nothing in Dettivo can fill the driver's cache ahead of that load.
- Not found: why the 2026-10-27 3.7 s take stalled after a 2.5 s take that morning ran in 32 ms. The store does not record the backend or whether the engine reloaded between them.
