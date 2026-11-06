# Release tail on a silent ending, before and after fn-93 (2026-11-06)

How long a Parakeet dictation's text takes after the release, measured through the daemon before and after fn-93 let a take whose last 150 ms before the release held no speech end at the release instead of after the 200 ms tail ([ADR 0093](../../adr/0093-a-take-runs-from-the-pre-roll-to-the-release-tail-and-a-tap-is-no-take.md#amendment-2026-11-06-a-silent-ending-ends-at-the-release)). A silent ending's text is ready 176 ms sooner at the median of paired takes, and a take released mid-word is as fast as before. Typed text gains less on short takes, because delivery waits until 250 ms after the stop before the first key. Every take is in [release-tail-2026-11-06.json](release-tail-2026-11-06.json), timings only.

## Method

- **Machine.** AMD Ryzen 9 7950X3D (32 threads), NVIDIA GeForce RTX 4090. Other processes, among them a game client and the installed daemon's Whisper engine, shared the GPU for the whole session, and the GPU idled at 210 MHz between takes.
- **Builds.** Release builds of `dettivod`, `dettivo`, `dettivo-engine-whisper` and `dettivo-engine-parakeet` with Vulkan, from `95a4f66` (before) and from fn-93's implementation (after), each built from its own tree into its own target directory. Block run 1 used the after build from before review round 1. That round moved the judged stretch to end at the release instead of the window boundary after it, which changes no take measured here: every silent ending's voice ended at least 280 ms before its release, and every speech ending's within 64 ms of it. The paired run, block run 2 and the 25 s takes use the build with the fix.
- **Daemons.** Isolated profiles under `scripts/qa/xvfb-session.sh`: private runtime, config, data and state directories, the socket inside the profile, the models hard-linked into a private tree. Parakeet Ultra, mode `raw`, `whisper/silero-vad` present, `DETTIVO_MOCK_MIC` playing a fixture and `DETTIVO_MOCK_INSERT` standing in for insertion. Both builds shared one driver shader cache.
- **Measure.** A client called `dictation.start`, waited the take length and called `dictation.stop`. Release-to-text runs from sending the stop to the `inserting` state event, the reading of ADR 0094. Typed text adds the completion's `insert_ms`, the delivery step with its 250 ms modifier settle (ADR 0090). `capture_ms` and `transcribe_ms` come from the completion change.
- **Takes.**
  - Silent endings on jfk stop in its pauses at 2.7, 4.9 and 7.9 s. Its floor there is about -39 dBFS, over the level stage's -50, so the voice check decides.
  - Silent endings on the level clip (jfk's first 2.3 s, then digital silence) stop at 2.7 s, where the level stage decides.
  - Speech endings stop jfk mid-word at 2.0, 6.5 and 9.5 s, and the level clip at 2.0 s.
- **Runs.** Each daemon ran two warm-up takes first.
  - The **paired run** kept a before and an after daemon side by side in one session. Every take ran on both, back to back, and the build that went first swapped each round: 7 rounds on jfk and 5 on the level clip.
  - **Block runs 1 and 2** ran one daemon at a time, alternating builds in blocks of 10 to 24 takes.

## Paired run

| Takes | Pairs | Release-to-text p50 (p10-p90, min-max) before, ms | After, ms | Paired difference p50, ms | `capture_ms` p50 (min-max), before and after | Typed text p50, before and after |
| --- | --- | --- | --- | --- | --- | --- |
| Silent ending, voice check (jfk) | 21 | 354 (292-397, 275-2,006) | 192 (112-217, 95-261) | -176, all 21 faster | 220 (220-220), 40 (40-60) | 354, 270 |
| Silent ending, level (digital silence) | 5 | 312 (298-354, 298-354) | 122 (115-148, 115-148) | -197, all 5 faster | 220 (220-221), 40 (40-40) | 312, 270 |
| Speech ending (jfk) | 21 | 357 (297-400, 282-405) | 359 (299-392, 296-453) | +2, 9 faster, 11 slower, 1 equal | 220 (220-220), 220 (220-221) | 357, 359 |
| Speech ending (level clip) | 5 | 300 (298-308, 298-308) | 294 (275-298, 275-298) | -9, all 5 faster | 220 (220-220), 220 (220-220) | 300, 294 |

- **Silent endings.** The after daemon logged `the take ends at the release` for all 26 of them and the before daemon for none. The 2,006 ms before-take is a `transcribe_ms` stall of 1,765 ms in the first round.
- **Speech endings.** `capture_ms` is the same in every pair. On jfk a two-sided sign test over the 20 pairs that differ gives p = 0.82.
- **Typed text.** On these 2 to 10 s takes `transcribe_ms` ran 54 to 118 ms at p50, so a silent ending's text was ready before the 250 ms settle ended and was typed at 270 ms. The keyboard gains only what the old release-to-text spent past that wait.

## Block runs

| Takes | Block run 1, n per build, before and after p50 (ms) | Block run 2, n per build, before and after p50 (ms) |
| --- | --- | --- |
| Silent ending, voice check (jfk) | 24: 278, 95 | 24: 354, 157 |
| Silent ending, level (digital silence) | 10: 265, 86 | 10: 424, 270 |
| Speech ending (jfk) | 60: 286, 283 | 60: 364, 425 |
| Speech ending (level clip) | 10: 263, 264 | 10: 431, 333 |

`capture_ms` was 220 or 221 ms before and 40 to 60 ms after on every silent ending of both runs, and 200 to 222 ms on every speech ending of both builds. Block run 1 ran on a quieter GPU, with `transcribe_ms` at 22 to 45 ms p50. In block run 2 it ran at 92 to 208 ms p50 in both builds and moved speech endings by +60 ms on jfk and -97 ms on the level clip, opposite ways with the same code path. That spread is the machine's, so the paired run decides.

## 25 s takes

Two blocks a build, alternating, 6 takes each of jfk played three times, stopped in a pause at 24.7 s or mid-word at 24.0 s.

| Takes | Release-to-text p50, before and after (ms) | Typed text p50, before and after (ms) | `capture_ms` p50, before and after |
| --- | --- | --- | --- |
| Silent ending | 316, 155 | 316, 270 | 221, 61 |
| Speech ending | 327, 424 | 327, 424 | 221, 221 |

The silent endings' typed text gains 46 ms, the part of the old release-to-text past the 250 ms settle. The speech endings ran under the same load as block run 2. Four of the six after-takes and two of the six before-takes had `transcribe_ms` spikes of 150 to 350 ms, against 72 to 94 ms otherwise, so six takes a build do not resolve them. The paired run above covers speech endings.

## How often a release ends in silence

The share comes from the 52 takes kept in one user's History. They were recorded before the 200 ms tail existed, so each ends at its release (their `capture_ms` was 27 to 83 ms). For each take, `feed_speech` with a threshold of 0.4 gave the end of its last voiced Silero window, and the level stage judged its last 100, 150 and 200 ms. Only these counts were kept.

| Stretch before the release | Silent by the level stage | Silent by the voice check | Silent endings |
| --- | --- | --- | --- |
| 100 ms | 46 | 4 | 50 of 52 |
| 150 ms | 42 | 4 | 46 of 52 |
| 200 ms | 36 | 2 | 38 of 52 |

The last 150 ms of those takes peaked between -89 and -33 dBFS, mostly under -60 dBFS, apart from two digitally silent endings, which is why the level stage settles most of them on that microphone. A microphone with a louder floor leaves more to the voice check. One user's takes are no population, so the share says more about this microphone than about dictation in general.
