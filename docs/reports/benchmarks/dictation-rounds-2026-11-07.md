# Dictation in rounds, before and after fn-92 (2026-11-07)

How long a Parakeet dictation's text takes after the release, and what its words cost in accuracy, when the take is transcribed in rounds while the key is held ([ADR 0102](../../adr/0102-dictation-decodes-in-rounds-while-the-key-is-held.md)). On an RTX 4090 a 120 s take shows its text 273 ms after the release at p50 against 841 ms before, a 60 s take 114 ms against 525 ms, and a 30 s take 88 ms against 349 ms. The word error rate of Parakeet Ultra and v3 in rounds is at or below the rate of one whole transcription on 26,000 English and 26,000 German reference words: the upper bound of every 95% interval is below 0, and Whisper large-v3-turbo is worse in rounds on English, so rounds stay off for it. Every take's timings and every WER summary are in [dictation-rounds-2026-11-07.json](dictation-rounds-2026-11-07.json).

## Release-to-text

### Method

- **Machine.** AMD Ryzen 9 7950X3D (32 threads), NVIDIA GeForce RTX 4090 on Vulkan. Other agents and the installed daemon shared the GPU (31 to 56% utilisation at the start and end of each run) and the CPU (load average 3.5 to 18).
- **Builds.** Release builds of `dettivod` and `dettivo` from `791554d` (before) and from fn-92's branch (after), with one pair of Vulkan engine binaries built from `791554d`; fn-92 changes no engine.
- **Daemon.** Isolated profiles under `scripts/qa/xvfb-session.sh`, with private runtime, config, data, state and cache directories, Parakeet Ultra, mode `raw`, `whisper/silero-vad` present, `DETTIVO_MOCK_MIC` playing ADR 0097's fixture (jfk repeated to 123 s) and `DETTIVO_MOCK_INSERT` standing in for insertion.
- **Measure.** The method of ADR 0094 and ADR 0097: a client calls `dictation.start`, waits the take length and calls `dictation.stop`. Release-to-text runs from sending the stop to the `inserting` state event. Each daemon ran two 5 s warm-up takes, then rounds of one 30 s, one 60 s and one 120 s take in that order.
- **Endings.** On this fixture the 30 s and 60 s takes stop in a pause of jfk, so the release tail ends early (`capture_ms` 41 to 64), and the 120 s takes stop mid-word and keep the whole 200 ms tail (`capture_ms` 224 to 227).

### Results

| Take | Before, `791554d`: p50 (min-max), ms | After, shipped build: p50 (min-max), ms | Spec R1: p50 at most 300 ms, p90 at most | `transcribe_ms` p50, before and after | Engine busy with rounds during the hold |
| --- | --- | --- | --- | --- | --- |
| 30 s | 349 (306-775), 6 takes | 88 (87-90), 5 takes | 489 ms | 290, 26 | 1,434-1,553 ms of 30 s (4.8-5.2%) |
| 60 s | 525 (449-649), 6 takes | 114 (113-115), 5 takes | 583 ms | 442, 30 | 4,780-5,037 ms of 60 s (8.0-8.4%) |
| 120 s | 841 (796-1,251), 6 takes | 273 (273-283), 5 takes | 890 ms | 619, 27 | 11,111-11,311 ms of 120 s (9.2-9.4%) |

- **R1 holds at every length.** With five takes the nearest-rank p90 is the slowest take: 90, 115 and 283 ms, each under the p50 that ADR 0097 recorded (489, 583 and 890 ms). The p50s are 88, 114 and 273 ms against the 300 ms bound.
- **What is left after the release.** `capture_ms` is the same in both builds (41, 63 and 225 ms at p50). The 120 s takes keep the 200 ms release tail because they end mid-word, so their text cannot arrive sooner than about 250 ms on this fixture. `transcribe_ms`, the gate and the tail decode together, fell from 290-619 ms to 26-30 ms and no longer grows with the take.
- **The release decode.** The shipped build decoded 5.3, 6.9 and 7.4 s of audio at the release for the three lengths, and the daemon logged 22 to 32 ms from the gate's verdict to the stitched words. No release waited for a round (`waited_ms` 0 in every take). Every round ran, none was skipped, and each 120 s take committed 197 words.
- **The text is the same.** Each after take typed as many characters as the before take of its length (288, 581 and 1,198).
- **Two earlier builds.** The first build, with an 8 s release window and the tail decode started after the gate, measured 124, 147 and 313 ms at p50 over 6 takes each; its 120 s takes missed the 300 ms bound by 13 ms. A second build moved the tail decode beside the gate and was stopped after one 120 s take at 301 ms. The shipped build starts the tail decode once the take's samples are final, overlapping the capture's stop and the take file's write, which took the 120 s p50 to 273 ms.
- **Before.** Two before takes stalled in the capture (`capture_ms` 333 ms at 30 s and 552 ms at 120 s); they are the maxima above and do not move the p50.

## Word error rate

### Method

`dettivo-qa bench windowed` (docs/qa.md) joined the first 400 FLEURS test utterances of English and German, as `scripts/qa/fetch-fleurs.sh` prepares them, in manifest order into takes of at least 30, 60 and 120 s. Each take was transcribed whole, padded as dictation pads a take, and in rounds through `dettivo_transcribe::rounds`: the rounds a held take of that length runs at the 1.5 s cadence, then the release with dictation's 4 s window and with the published scheme's 8 s window, both after the same rounds. The rate is the corpus word error rate over FLEURS's normalised transcription. The interval is a 95% paired bootstrap of the windowed rate minus the whole rate, resampling takes, 10,000 times. Every take decoded its tail, because the bench does not simulate the speech check's silence shortcut. The run predates the second pass that fn-92's review added for a padded clip that comes back empty; none of the run's whole decodes came back empty. Each utterance appears once per take length, so the pooled row counts each utterance three times and its interval is narrower than three independent sets would give.

### Parakeet, the shipped 4 s window

| Model, language | Takes (30 / 60 / 120 s) | Whole WER | Windowed WER | Difference | 95% CI |
| --- | --- | --- | --- | --- | --- |
| Ultra, en | 111 / 59 / 31 | 0.0571 | 0.0552 | -0.0019 | -0.0035 to -0.0005 |
| Ultra, de | 139 / 79 / 41 | 0.0469 | 0.0453 | -0.0016 | -0.0028 to -0.0004 |
| v3, en | 111 / 59 / 31 | 0.2213 | 0.1342 | -0.0871 | -0.1079 to -0.0681 |
| v3, de | 139 / 79 / 41 | 0.0938 | 0.0726 | -0.0212 | -0.0330 to -0.0104 |

Per take length:

| Model, language | 30 s | 60 s | 120 s |
| --- | --- | --- | --- |
| Ultra, en | -0.0016 (-0.0054 to +0.0010) | -0.0006 (-0.0023 to +0.0010) | -0.0035 (-0.0063 to -0.0009) |
| Ultra, de | +0.0000 (-0.0018 to +0.0017) | -0.0005 (-0.0024 to +0.0015) | -0.0044 (-0.0067 to -0.0022) |
| v3, en | -0.0303 (-0.0578 to -0.0053) | -0.1291 (-0.1666 to -0.0936) | -0.1024 (-0.1395 to -0.0673) |
| v3, de | +0.0021 (-0.0015 to +0.0068) | -0.0185 (-0.0384 to -0.0020) | -0.0475 (-0.0747 to -0.0223) |

- Every pooled upper bound is below 0, and every per-length bound is at or below +0.0017 except v3 German at 30 s, whose upper bound is +0.0068.
- Parakeet v3's whole-take rate on these joined takes is far above Ultra's, 0.2616 against 0.0608 on the English 120 s takes and 0.2837 against 0.0562 at 60 s, and the rounds recover most of the words it loses, so its differences are negative. This run does not establish why the whole decode loses them.
- The 8 s window scores within 0.0016 of the 4 s window on every pooled row (Ultra en -0.0022, Ultra de -0.0013, v3 en -0.0866, v3 de -0.0201).

### Whisper large-v3-turbo

The Whisper run used `--quick`, the first 100 utterances of each language, and was stopped once its German 30 s takes were done and 22 of its 60 s takes, to free the GPU for the Parakeet runs; its English set is complete.

| Language, window | Takes | Whole WER | Windowed WER | Difference | 95% CI |
| --- | --- | --- | --- | --- | --- |
| en, 4 s | 49 (28 / 14 / 7) | 0.0919 | 0.1268 | +0.0349 | -0.0037 to +0.0755 |
| en, 8 s | 49 | 0.0919 | 0.1300 | +0.0381 | -0.0015 to +0.0796 |
| de, 4 s | 60 (38 / 22 / 0) | 0.0527 | 0.0510 | -0.0017 | -0.0052 to +0.0019 |
| de, 8 s | 60 | 0.0527 | 0.0436 | -0.0091 | -0.0184 to -0.0027 |

The English 120 s takes carry most of it: +0.1001 at 4 s over 7 takes, with whole 0.1061 against windowed 0.2062.
