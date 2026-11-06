# Release tail on a silent ending, before and after fn-93 (2026-11-06)

How long a Parakeet dictation's text takes to appear after the release, measured through the daemon before and after fn-93 let a take whose last 150 ms held no speech end at the release instead of after the 200 ms tail ([ADR 0093](../../adr/0093-a-take-runs-from-the-pre-roll-to-the-release-tail-and-a-tap-is-no-take.md#amendment-2026-11-06-a-silent-ending-ends-at-the-release)). A silent ending's text now arrives 183 ms sooner at p50, and a take released mid-word is as fast as before. Every take is in [release-tail-2026-11-06.json](release-tail-2026-11-06.json), timings only.

## Method

- **Machine.** AMD Ryzen 9 7950X3D (32 threads), NVIDIA GeForce RTX 4090. Other processes, among them a game client and the installed daemon's Whisper engine, loaded the GPU between 2 and 97% during the run, at a 1-minute load average of 1 to 13.
- **Builds.** Release builds of `dettivod`, `dettivo`, `dettivo-engine-whisper` and `dettivo-engine-parakeet` with Vulkan, from `95a4f66` (before) and from fn-93's implementation (after), each built from its own tree into its own target directory.
- **Daemon.** One isolated profile per run under `scripts/qa/xvfb-session.sh`: private runtime, config, data and state directories, the socket inside the profile, the models hard-linked into a private tree. Parakeet Ultra, mode `raw`, `whisper/silero-vad` present, `DETTIVO_MOCK_MIC` playing a fixture and `DETTIVO_MOCK_INSERT` standing in for insertion. Both builds shared one driver shader cache.
- **Measure.** A client called `dictation.start`, waited the take length and called `dictation.stop`. Release-to-text runs from sending the stop to the `inserting` state event, the reading of ADR 0094. `capture_ms` and `transcribe_ms` come from the completion change.
- **Takes.** Each daemon ran two warm-up takes, then rounds of every listed take in turn. The builds alternated: four blocks per build on jfk and two on the level clip.
  - Silent endings on jfk stop in its pauses at 2.7, 4.9 and 7.9 s. Its floor there is about -39 dBFS, over the level stage's -50, so the voice check decides.
  - Silent endings on the level clip (jfk's first 2.3 s, then digital silence) stop at 2.7 s, where the level stage decides.
  - Speech endings stop jfk mid-word at 2.0, 6.5 and 9.5 s, and the level clip at 2.0 s.

## Results

| Takes | n per build | Release-to-text p50 (p10-p90, min-max) before, ms | After, ms | `capture_ms` p50 (min-max), before and after |
| --- | --- | --- | --- | --- |
| Silent ending, voice check (jfk) | 24 | 278 (265-312, 264-364) | 95 (84-146, 82-265) | 220 (220-220), 40 (40-60) |
| Silent ending, level (digital silence) | 10 | 265 (265-302, 263-302) | 86 (84-179, 82-179) | 220 (220-220), 40 (40-40) |
| Speech ending (jfk) | 60 | 286 (264-383, 262-595) | 283 (264-298, 262-682) | 220 (220-221), 220 (220-221) |
| Speech ending (level clip) | 10 | 263 (263-312, 262-312) | 264 (262-351, 262-351) | 220 (220-220), 220 (220-220) |

- **Silent endings.** p50 saves 183 ms through the voice check and 179 ms through the level stage, over the 150 ms the spec asks for. The daemon log shows `the take ends at the release` for every one of the 34 silent endings after the change. The slowest after-takes (183 and 265 ms) are `transcribe_ms` spikes of 102 and 204 ms; their capture was 60 and 40 ms.
- **Speech endings.** `capture_ms` is 220-221 ms in all 70 takes of each build, so the tail is unchanged. The p50s differ by -3 ms on jfk and +1 ms on the level clip. The before build's own jfk block p50s ran from 281 to 326 ms. The slowest takes of both builds (563-682 ms after, 434-595 ms before) are `transcribe_ms` spikes that came with the GPU load.

## How often a release ends in silence

The share comes from the 52 takes kept in one user's History. They were recorded before the 200 ms tail existed, so each ends at its release (their `capture_ms` was 27 to 83 ms). For each take, `feed_speech` with a threshold of 0.4 gave the end of its last voiced Silero window, and the level stage judged its last 100, 150 and 200 ms. Only these counts were kept.

| Stretch before the release | Silent by the level stage | Silent by the voice check | Silent endings |
| --- | --- | --- | --- |
| 100 ms | 46 | 4 | 50 of 52 |
| 150 ms | 42 | 4 | 46 of 52 |
| 200 ms | 36 | 2 | 38 of 52 |

The last 150 ms of those takes peaked between -89 and -33 dBFS, mostly under -60 dBFS, apart from two digitally silent endings, which is why the level stage settles most of them on that microphone. A microphone with a louder floor leaves more to the voice check. One user's takes are no population, so the share says more about this microphone than about dictation in general.
