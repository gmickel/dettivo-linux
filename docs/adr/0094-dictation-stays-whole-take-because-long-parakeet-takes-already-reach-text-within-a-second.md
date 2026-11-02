# 0094. Dictation stays whole-take, because Parakeet takes up to a minute already reach text within a second

Status: Accepted 2026-11-02. Closes fn-86 at its measurement gate.

## What this gives you

A 60 s Parakeet dictation on the reference GPU shows its text 0.68 s after you release the key, measured through the daemon. That includes the 200 ms release tail. Dictation keeps one recognition pass over the whole take. Streaming during the hold, live partial text in the pill and the stream fallbacks are not built.

## Situation

fn-86 proposed streaming Parakeet while the key is held, so that only the last second of a long take is left to transcribe after the release. The spec made that work conditional on one measurement. Streaming is built only if release-to-text for takes of 30 s or longer exceeds 1 s on the reference GPU. Its R2 then holds streaming to a p50 under 1 s for a 60 s take.

The spec names no statistic, no start and end points and no longest take. This record reads the gate as follows.

- **Release-to-text** runs from the release to the moment the text is ready, which is the session's `inserting` state. It includes the 200 ms release tail (0093), the capture finish, the speech gate (0081), recognition and the text step. It excludes insertion, because streaming would not change how long typing or pasting takes.
- **The statistic is the warm p50**, the statistic R2 uses. A take that first had to load the engine is reported apart, because streaming does not remove that load.
- **The lengths are 30 s and 60 s**, the gate's lower bound and R2's reference take. Two-minute takes are measured and reported, and the decision below names what they need.
- **The reference GPU** is the NVIDIA RTX 4090 on Vulkan with the release Parakeet engine (`cargo build --release -p dettivo-engine-parakeet --features vulkan`).

All measurements were taken on 2026-11-02 on that machine, which has 32 threads. Other agents shared the GPU at 4-53 % utilisation and the CPU at a load average of 5-20. The Vulkan figures were stable across repetitions. The CPU figures are contended and are upper bounds.

### Release-to-text through the daemon

`dettivod` ran as a release build in an isolated profile under `scripts/qa/xvfb-session.sh`, with Parakeet Ultra on Vulkan, mode `raw`, the Silero gate model present, and `DETTIVO_MOCK_MIC` playing `jfk.wav` repeated to the take length. A client called `dictation.start`, waited for the take length and called `dictation.stop`. Release-to-text is the time from the stop call to the `inserting` state event. The first take after the engine loaded is excluded.

| Take | Runs | Release-to-text (ms) | `capture_ms` | `transcribe_ms` |
| ---- | ---- | -------------------- | ------------ | --------------- |
| 30 s | 2 | 594, 657 | 221 | 353, 416 |
| 60 s | 3 | 676, 683, 731 (p50 683) | 222-223 | 433-488 |
| 120 s | 2 | 1,079, 1,234 | 225 | 834, 989 |

`capture_ms` is the 200 ms release tail plus the capture finish. `transcribe_ms` covers the speech gate, recognition, padding, transport and the text step.

### Each stage alone, per engine and backend

The engine binaries ran in protocol mode with the model loaded once. Each figure is the p50 of five warm requests (three on the CPU, for Whisper and for Silero) over the repeated fixture. The first request at each length was discarded.

| Stage, model | Backend | 5 s | 15 s | 30 s | 60 s | 90 s | 120 s |
| ------------ | ------- | --- | ---- | ---- | ---- | ---- | ----- |
| Parakeet Ultra | Vulkan | 33 | 56 | 162 | 237 | 360 | 366 |
| Parakeet v3 | Vulkan | 26 | 52 | 125 | 280 | 328 | 434 |
| Parakeet v2 | Vulkan | 24 | 59 | 97 | 191 | 327 | 427 |
| Parakeet Ultra | CPU | 174 | 466 | 952 | 2,280 | - | - |
| Parakeet v3 | CPU | 229 | 516 | 1,023 | 2,282 | - | - |
| Whisper large-v3-turbo | Vulkan | 107 | 146 | 267 | 571 | - | - |
| Whisper large-v3-turbo | CPU | 11,478 | - | 12,803 | - | - | - |
| Silero speech gate (`detect_speech`) | CPU | 12 | - | 81 | 156 | - | 336 |

All figures are milliseconds. Whisper on the CPU takes as long for 5 s as for 30 s because it pads every window to 30 s, and its two figures are single runs. Takes over 90 s go through Parakeet in two overlapping 60 s chunks (`align::chunks`).

### History

The local History store holds 93 Parakeet dictations with a `timings` block, recorded by the installed daemon between 2026-09-14 and 2026-10-30. That daemon had neither the speech gate nor the release tail. Of the 93, 20 are 30 s or longer.

- 18 of those 20 have `transcribe_ms` from 121 to 715 ms, median 363 ms. The longest take, 142.9 s, took 715 ms.
- The other two, 49.4 s and 69.6 s long, took 2,765 and 3,862 ms. Across all 93 takes, seven exceeded 1 s, at 1.1-3.9 s. They ran 0.1 s to 69.6 s long. Three were the first take after more than 300 s of idle, which unloads the engine (`stt_idle_seconds`). The other four came 25-107 s after the take before them, and the takes around them of similar length took 110-369 ms. The store does not record why those four stalled.

## Decision

- **The gate is not met.** On the reference GPU, release-to-text has a p50 of 683 ms for a 60 s take, which already meets R2's target without streaming. Every take of 30 to 60 s measured stays at or under 731 ms.
- Dictation keeps the whole-take path for every engine and backend. fn-86's streaming requests (`stream.open`, `stream.feed`, `stream.finish`, `stream.cancel`), pause capping, the `dictation.partial` event, the pill's live text and the stream fallback reasons are not added.
- Meeting live windows keep their scheduling. The yield-to-dictation rule existed only to protect a dictation stream.
- **Two-minute takes are the bound.** They reach text in 1,079-1,234 ms, 80-230 ms over 1 s. The Silero speech gate runs over the whole take before recognition starts, and costs 336 ms at 120 s. That is more than the overshoot. Running the gate during the hold, or beside recognition, would bring these takes under 1 s without an engine stream. That change is left for its own spec.

## Consequences

- After the release, a take on the reference GPU waits about 0.2 s of release tail, 2.6-2.8 ms per second of audio for the speech gate, and 3-5 ms per second of audio for recognition. This split comes from the two tables and was not profiled inside the daemon.
- Release-to-text grows with the take length and passes 1 s at about two minutes. Re-run this measurement if a slower GPU becomes the reference, or if two-minute dictations become common before the gate moves off the serial path.
- On the CPU, recognition alone passes 1 s at 30 s and reaches 2.3 s at 60 s. fn-86 ruled out CPU streaming, because re-encoding overlapping windows costs more than it saves, and this record leaves that unchanged.
- The first take after the engine went idle still waited 1.1-3.1 s in History, which includes loading the model. Stalls of 1.7-3.9 s also hit four warm takes there, with no recorded cause. Their size does not follow the take length, so streaming the recognition would not remove them. This record does not address either.
