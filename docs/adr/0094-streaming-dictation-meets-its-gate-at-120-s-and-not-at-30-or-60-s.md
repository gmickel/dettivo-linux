# 0094. Streaming dictation meets its gate at 120 s and not at 30 s or 60 s

Status: Accepted 2026-11-02 by Gordon Mickel. Records fn-86's measurement gate (R1) and closes fn-86 without streaming; fn-90 takes on the two-minute miss.

## What this gives you

A Parakeet dictation of up to a minute on the reference GPU already shows its text within 0.73 s of the release, measured through the daemon. A two-minute take waits 1.08-1.23 s. fn-86 makes streaming conditional on long takes passing 1 s, so of the three measured lengths, 30 s, 60 s and 120 s, only the two-minute take meets the gate. Release-to-text was not measured between 60 s and 120 s, so this record sets no boundary inside that range. Dictation stays offline: streaming is not built. The two-minute miss goes to fn-90, which runs the speech check during the take. This record gives the numbers, the reason the pinned engine cannot stream the dictation models as the spec assumes, and why the smaller change won.

## Situation

fn-86 proposed streaming Parakeet while the key is held, so that only the last second of a long take is left to transcribe after the release. The spec makes that work conditional on one measurement. Streaming is built only if release-to-text for takes of 30 s or longer exceeds 1 s on the reference GPU. Its R2 then holds streaming to a p50 under 1 s for a 60 s take.

This record reads the gate's terms as follows.

- **Release-to-text** runs from the release to the moment the text is ready, which is the session's `inserting` state. It includes the 200 ms release tail (0093), the capture finish, the speech gate (0081), recognition and the text step. It excludes insertion, because streaming would not change how long typing or pasting takes.
- **The statistic is the warm p50**, the statistic R2 uses. A take that first had to load the engine is reported apart, because streaming does not remove that load.
- **"30 s or longer" has no upper bound.** Any take length from 30 s up whose p50 passes 1 s meets the gate.
- **The reference GPU** is the NVIDIA RTX 4090 on Vulkan with the release Parakeet engine (`cargo build --release -p dettivo-engine-parakeet --features vulkan`).

All measurements were taken on 2026-11-02 on that machine, which has 32 threads. Other agents shared the GPU at 4-53 % utilisation and the CPU at a load average of 5-20. The Vulkan figures were stable across repetitions. The CPU figures are contended and are upper bounds.

### Release-to-text through the daemon

`dettivod` ran as a release build in an isolated profile under `scripts/qa/xvfb-session.sh`, with Parakeet Ultra on Vulkan, mode `raw`, the Silero gate model present, and `DETTIVO_MOCK_MIC` playing `jfk.wav` repeated to the take length. A client called `dictation.start`, waited for the take length and called `dictation.stop`. Release-to-text is the time from the stop call to the `inserting` state event. The first take after the engine loaded is excluded.

| Take | Runs | Release-to-text (ms) | `capture_ms` | `transcribe_ms` |
| ---- | ---- | -------------------- | ------------ | --------------- |
| 30 s | 2 | 594, 657 | 221 | 353, 416 |
| 60 s | 3 | 676, 683, 731 (p50 683) | 222-223 | 433-488 |
| 120 s | 2 | 1,079, 1,234 (p50 1,157) | 225 | 834, 989 |

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

All figures are milliseconds. Whisper on the CPU takes as long for 5 s as for 30 s because it pads every window to 30 s, and its two figures are single runs. Parakeet takes a take of up to 90 s in one pass. A longer take goes through windows of at most 60 s that overlap by 2 s (`align::chunks`), so a 120 s take runs as three windows: 0-60 s, 58-118 s and 116-120 s.

### History

The local History store holds 93 Parakeet dictations with a `timings` block, recorded by the installed daemon between 2026-09-14 and 2026-10-30. That daemon had neither the speech gate nor the release tail. Of the 93, 20 are 30 s or longer, and 2 are longer than 90 s (100.1 s and 142.9 s).

- 18 of the 20 have `transcribe_ms` from 121 to 715 ms, median 363 ms. The 142.9 s take took 715 ms.
- The other two, 49.4 s and 69.6 s long, took 2,765 and 3,862 ms. Across all 93 takes, seven exceeded 1 s, at 1.1-3.9 s, and they ran 0.1 s to 69.6 s long. Three were the first take after more than 300 s of idle, which unloads the engine (`stt_idle_seconds`). The other four came 25-107 s after the take before them, and the takes around them of similar length took 110-369 ms. The store does not record why those four stalled.

### What the pinned engine can stream

parakeet.cpp v0.5.0 has a streaming C API (`parakeet_capi_stream_begin`, `_feed`, `_finalize`), but only for cache-aware streaming models such as `parakeet_realtime_eou_120m-v1`. `parakeet_capi_stream_begin_lang` returns `model is not a cache-aware streaming model` when the GGUF carries no `parakeet.streaming.*` keys. None of the three dictation models (`tdt-0.6b-v2-q8_0.gguf`, `tdt-0.6b-v3-q8_0.gguf`, `parakeet-ultra.q8_0.gguf`) carries any. The spec's "Parakeet buffers its own context" therefore has no engine support for these models today.

## Decision

fn-86 closes with this record, and streaming is not built. Gordon Mickel chose option B on 2026-11-02. fn-90 carries its follow-on work: run the Silero speech check incrementally during the take, so its verdict is ready at the release, and find and fix the stalls of the first take after idle.

- **Of the measured lengths, the gate is met at 120 s only.** Release-to-text has a p50 of 1,157 ms at 120 s. At 30 s and 60 s it stays at or under 731 ms, and the 60 s p50 of 683 ms already meets R2's target without streaming. Takes between 60 s and 120 s were not measured, so where the 1 s line falls in that range is unknown.
- **Option A, build streaming as specified (not chosen).** Streaming the TDT models means writing a buffered decoder in `dettivo-engine-parakeet`. Every 1.04 s it would re-run the offline model over a window of 5.6 s of left context, the 1.04 s chunk and 1.04 s of lookahead, keeping the words whose timestamps fall in the chunk. The alternative is a cache-aware model, which changes the dictation model and is a product choice. On top of the decoder, R2-R7 add the four `stream.*` requests, the session feed during the hold, pause capping, the four fallbacks with a persisted `fallback` reason in History, the `dictation.partial` event, the pill's two-weight text, the meeting yield rule and their tests and docs. After the release, a two-minute take would still wait for the 0.2 s tail and the 0.34 s Silero pass over the whole take.
- **Option B, close fn-86 and move the speech gate off the serial path (chosen).** The Silero pass runs over the whole take before recognition starts and costs 336 ms at 120 s, more than the 80-230 ms by which two-minute takes miss 1 s. Running it during the hold, or beside recognition, would bring those takes under 1 s without a stream. That change is fn-90. Live partial text is not built.
- B was recommended and chosen. Two-minute takes are 2 of 93 takes in History, takes of up to a minute already meet R2, and A adds a decoder and protocol surface whose accuracy the WER fixtures have not judged.

## Consequences

- Dictation keeps the whole-take path for every engine and backend. Parakeet recognises a take after the release, not while the key is held, and the pill shows no partial text.
- After the release, a take on the reference GPU waits about 0.2 s of release tail, 2.6-2.8 ms per second of audio for the speech gate and 3-5 ms per second of audio for recognition. This split comes from the two tables and was not profiled inside the daemon.
- On the CPU, recognition alone passes 1 s at 30 s and reaches 2.3 s at 60 s. fn-86 rules out CPU streaming, because re-encoding overlapping windows costs more than it saves, and either option keeps that.
- The first take after the engine went idle waited 1.1-3.1 s in History, which includes loading the model. Stalls of 1.7-3.9 s also hit four warm takes there, with no recorded cause. Their size does not follow the take length, so streaming would not have removed them. fn-90 investigates the stalls of the first take after idle.
