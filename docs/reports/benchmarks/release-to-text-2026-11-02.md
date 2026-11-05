# Release-to-text before and after fn-90 (2026-11-02)

How long a Parakeet dictation's text takes to appear after the release, measured through the daemon before and after fn-90 moved the speech check into the take ([ADR 0097](../../adr/0097-the-speech-check-hears-the-take-while-it-is-captured.md)) and lengthened Parakeet's warm-up ([ADR 0098](../../adr/0098-parakeet-warms-up-on-its-longest-single-pass.md)). Every take is in [release-to-text-2026-11-02.json](release-to-text-2026-11-02.json), timings only.

## Method

- **Machine.** AMD Ryzen 9 7950X3D (32 threads), NVIDIA GeForce RTX 4090, driver 615.78.08. Another agent's Whisper engine shared the GPU, and the 1-minute load average ran from 3 to 22.
- **Builds.** Release builds of `dettivod`, `dettivo`, `dettivo-engine-whisper` and `dettivo-engine-parakeet` with Vulkan, from `1dbe657` (before) and from fn-90's implementation commit (after).
- **Daemon.** An isolated profile under `scripts/qa/xvfb-session.sh` with the models hard-linked into a private tree: Parakeet Ultra, mode `raw`, `whisper/silero-vad` present. `DETTIVO_MOCK_MIC` played the jfk fixture repeated to length, and `DETTIVO_MOCK_INSERT` stood in for insertion.
- **Measure.** A client called `dictation.start`, waited the take length and called `dictation.stop`. Release-to-text runs from sending the stop to the `inserting` state event, the reading of ADR 0094.
- **Cache.** Both builds ran on one driver shader cache that the engine probes of ADR 0098 had already filled, the state of a machine whose driver and engine have not changed since their last use.

## Warm takes

Five takes at each length after two warm-up takes, one daemon per build.

| Take | Before p50 (ms) | Before, every take | After p50 (ms) | After, every take |
| --- | --- | --- | --- | --- |
| 30 s | 570 | 570, 15,589, 14,957, 489, 455 | 489 | 426, 388, 502, 511, 489 |
| 60 s | 722 | 711, 730, 767, 722, 717 | 583 | 583, 645, 639, 507, 549 |
| 120 s | 1,271 | 1,178, 1,389, 1,271, 1,313, 1,221 | 890 | 747, 961, 587, 958, 890 |

The two 15 s takes before came at a load average of 15 with nothing in the daemon log between `transcribing` and `inserting`. They are contention. Without them the before p50 at 30 s is 489 ms (455-570), the same as after, so this run does not resolve a gain at 30 s; the pass it removes cost 81 ms at 30 s in ADR 0094's engine measurement. ADR 0094 measured 594-657 ms at 30 s, 683 ms at 60 s and 1,157 ms at 120 s on the same machine on a quieter run.

## The first take after the engine idled out

One daemon per build with `stt_idle_seconds = 61`, so the reload comes more than 60 s after the last recognition and its warm-up runs, as it does at the default 300 s. Each cycle took three 3 s takes, then waited for the idle reaper to unload the engine (73-76 s idle). The first take of each cycle after the first is the first take after an unload. The preload at the press loaded the model in 573-682 ms and finished the warm-up by 663-927 ms into the take, before the release at 3 s.

| Build | Warm-up after a reload | First take after the unload (ms) | The two takes after it (ms) |
| --- | --- | --- | --- |
| Before (30 s warm-up) | 78-85 ms | 278, 278, 277, 275, 279 | 461, 401, 456, 439, 434, 462, 378, 476, 442, 314 |
| After (90 s warm-up) | 237-248 ms | 266, 267, 266, 266, 266 | 458, 393, 379, 361, 385, 358, 388, 389, 481, 268 |

On a current shader cache the first take after an unload is no slower than a warm take in either build, so fn-90's R4 bound of 20% holds. The takes right after a reload ran faster than the later ones in both builds. The run does not show why, and both builds show the same pattern.

A run with `stt_idle_seconds = 5` is in the JSON as `idle_unload_5s_no_warm_up`. The reload there came within 60 s of the last recognition, so `WARM_UP_SKIP` skipped the warm-up. Its first takes after an unload were 290-308 ms before (one 2,107 ms take at a load average of 22) and 276-501 ms after.

## Where the stalls came from

The engine probes behind ADR 0098 time `dettivo-engine-parakeet` alone on an empty driver shader cache. With the 30 s warm-up, the first 45 s take took 2,476-2,867 ms and the first 90 s take 609-625 ms. With the 90 s warm-up, every length from 3 s to 120 s ran at its warm time. ADR 0098 holds the table.
