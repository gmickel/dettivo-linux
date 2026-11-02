# 0097. The speech check hears the take while it is captured

Status: Accepted 2026-11-02. Amends 0081.

## What this gives you

A dictation's text no longer waits for the speech gate to listen to the whole take after the release. The Silero voice check now runs while you speak, so at the release only the last fraction of a second is left for it. On the reference GPU a two-minute Parakeet take now shows its text 890 ms after the release at p50, against 1,271 ms before, and a one-minute take's 583 ms against 722 ms. The gate's verdict is the one it gave before, down to the milliseconds of speech it counts.

## Situation

fn-79 put a speech gate in front of every dictation (ADR 0081). Its second stage, the Silero voice activity model in a CPU-only `dettivo-engine-whisper` process, ran over the whole take after the release and before recognition started. ADR 0094 measured that pass at 2.6-2.8 ms per second of audio, 336 ms for a two-minute take. Release-to-text was 1,079-1,234 ms there against a 1 s goal.

Silero is a recurrent model: whisper.cpp runs it over 512-sample windows (32 ms), and each window's speech probability depends on every window before it through the LSTM state. `whisper_vad_detect_speech` resets that state on every call, so judging a take in pieces with the stock API gives different probabilities from judging it whole. Measured on the jfk fixture, resetting the state for every piece reports 0 ms of speech where the whole take has 7,600 ms.

## Decision

The voice check carries its state across pieces, and the verdict is computed from every window's probability exactly as before.

- **whisper.cpp patch.** The vendored source (`third_party/whisper-rs-sys`, alongside the ADR 0080 patch) runs `whisper_vad_detect_speech`'s body with a `reset` flag. The new `dettivo_vad_detect_speech_more` runs it without one: the state carries over and the new windows' probabilities are appended. Fed whole windows, the stream ends with the probabilities one call over the whole take computes, and `whisper_vad_segments_from_probs` turns them into the same segments.
- **Protocol.** `feed_speech` (one `pcm16k` attachment, `start` for a new take) runs the whole windows of what has arrived and holds the rest. `detect_speech` with `continues` set to the samples fed so far runs the final samples and answers `speech_ms` for the whole take. A `continues` that does not match the stream, or a feed without a started stream, is `bad_request`.
- **Session.** Once the press has lasted the minimum hold (ADR 0093), the worker hands the detector's stream to a `speech-check` thread and sends it everything captured so far. A tap never reaches the detector. From then on, the recording loop sends each captured chunk to the thread over a channel and never waits on it. A check that falls behind runs its backlog in one piece. Audio that arrives after the stop is seen can still be cut from the take by the release tail (ADR 0093), so it is never fed. At the release the thread stops after the piece it is running, and the take's remaining samples go to `detect_speech` with `continues`. A take that ends without a verdict, such as a cancel or a lost device, also waits for the piece the thread is running before the session ends. The voice process holds one take's state, so a piece left running would reset the next take's stream.
- **Fallback.** When a feed failed, or the check ran samples the take was later cut before, the release runs the whole take through `detect_speech` as before, and a failing pass still leaves the level stage to decide (ADR 0081).
- **The level stage stays at the release.** Its DC offset is the whole take's mean, so it cannot be judged in pieces without changing its arithmetic. It costs 3.4 ms over a 120 s take in a release build.

The thresholds do not change: -50 dBFS on the loudest 30 ms window, Silero threshold 0.4 and 160 ms of speech.

## Consequences

Release-to-text through the daemon, before (`1dbe657`) and after this change, on 2026-11-02. Both are release builds with Parakeet Ultra on Vulkan, measured with the method of ADR 0094: an isolated profile under `scripts/qa/xvfb-session.sh`, `DETTIVO_MOCK_MIC` playing the jfk fixture repeated to length, and the time from `dictation.stop` to the `inserting` state event. Both runs share one driver shader cache that the probes of ADR 0098 had already filled. Each figure is the p50 of five warm takes. The RTX 4090 was shared with another agent's Whisper engine, at a load average of 6 to 15. [The benchmark report](../reports/benchmarks/release-to-text-2026-11-02.md) lists every take.

| Take | Before (ms) | After (ms) | `transcribe_ms` p50, before and after |
| --- | --- | --- | --- |
| 30 s | 570 with two contended takes of 15.0 and 15.6 s; 489 (455-570) without them | 489 (388-511) | 327, 247 |
| 60 s | 722 (711-767) | 583 (507-645) | 478, 339 |
| 120 s | 1,271 (1,178-1,389) | 890 (587-961) | 1,024, 643 |

`capture_ms`, the release tail and the capture finish, stayed at 221-226 ms in both. The gain grows with the take: 139 ms at 60 s and 381 ms at 120 s, about the cost ADR 0094 measured for the Silero pass at those lengths (156 and 336 ms). At 30 s the pass cost 81 ms there, and this run does not resolve it: without the two contended takes the before p50 equals the after p50.

- The verdict is unchanged. `crates/dettivo-engine-whisper/tests/vad_stream.rs` feeds every gate fixture (speech, speech 24 dB down, one short word, silence, steady noise, a mains hum, and two sentences around a pause) in 20 ms pieces with the 200 ms tail held back, in 100-sample pieces, in uneven 7,777-sample pieces, and whole. Each finds exactly the speech the whole-take pass finds. With the state reset on every piece, the same test fails at 0 against 7,600 ms. fn-79's gate tests and fixtures pass unmodified.
- The capture never waits for the check. `crates/dettivo-session/tests/speech_check.rs` feeds a 10 s take while each check piece takes 200 ms. The take holds every sample, the check heard all of them in order, and the release waited for at most one piece.
- The voice process now runs during every take, about 3 ms of one CPU core per second of audio, and the check judges takes the level stage then finds silent, which it skipped before. It still never touches the GPU.
- The VAD process starts at the first chunk after the minimum hold rather than at the release, so its spawn (about 6 ms) is off the release path too. `speech_check.rs` also covers the tap, the audio from before the minimum hold, and a take after a lost device.
- `whisper.h` and the Rust bindings stay as published: the new function is defined `extern "C"` in the patched `whisper.cpp` and declared in `crates/dettivo-engine-whisper/src/vad.rs`, which now holds the VAD context through `whisper-rs`'s raw API instead of `WhisperVadContext`. Moving to a new whisper.cpp release means applying both patch blocks again (`third_party/whisper-rs-sys/README.md`).
