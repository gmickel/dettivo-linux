# 0093. A take runs from the pre-roll to the release tail, and a tap is no take

Status: Accepted 2026-11-02. Amends 0082 (where a take ends) and 0083 (how a tap ends). Amended 2026-11-06 (fn-93): a take whose last 150 ms held no speech ends at the release ([below](#amendment-2026-11-06-a-silent-ending-ends-at-the-release)).

## What this gives you

A last word still sounding when you let go of the key is in the take, because recording continues for 200 ms after the release. A key you only tapped never reaches the speech engine, plays no stop cue and leaves nothing in History.

## Situation

- 0082 ended the capture at the stop request, on the stopping thread. That keeps a key-up click out of the take, and it also cuts the end of a word the user released the key on.
- 0083's hold machine already drops a take released under 300 ms, through `dictation.cancel`'s path. That take ended `cancelled` with reason `cancelled` and played the stop cue.
- The spec asks for `press_time` to come from a `--pressed-at` monotonic timestamp the CLI fills in. fn-82 already times every edge by when it reached the daemon (`dictation.press` and `dictation.release` take `Instant::now()` before they parse their parameters), and a press that queues behind a slow start keeps its arrival time.

## Decision

- **The press time is when the CLI saw the key.** `dettivo dictation press`, `release` and `toggle`, the verbs the compositor bindings run, read `CLOCK_MONOTONIC` as the command starts and send it as `at_ns`; the daemon counts the edge from that moment, so a call that waited before the daemon got to it is still timed from the key. A value from the future or older than 2 s (a clock from another time namespace) falls back to the arrival. `dictation.start`, `dictation.stop` and the portal and evdev edges pass the `Instant` the call or event reached the daemon. `Session::start` takes the time as `pressed_at` and `Snapshot.started` carries it. The gap left between the key and the press time is the compositor's exec of the CLI, a few milliseconds.
- **Pre-roll.** With the always-ready microphone open (0092), the take starts at `pressed_at − 450 ms` (`PRE_ROLL`). With it off, the take starts with the first chunk the new capture delivers.
- **Release tail.** `Session::stop_expected(job, released_at)` records the release time and the worker keeps recording. It stops the source once the source delivered audio captured at or after `released_at + 200 ms` (`POST_RELEASE_TAIL`), or once no audio arrived for 100 ms (`TAIL_STALL_GRACE`), so a stalled device closes the take after the grace. A take whose last 150 ms before the release held no speech skips the tail ([amendment](#amendment-2026-11-06-a-silent-ending-ends-at-the-release)).
- **The take ends at the tail by capture time.** The worker keeps every chunk with its capture stamp and writes the take once, after cutting it at `released_at + 200 ms` to the sample. A release that queued behind a slow start, or a ring backlog delivered after the release, never adds audio past the tail to the take file or the engine. A chunk captured before the cut but delivered while the source stops still joins the take, which keeps 0082's one-quantum tolerance at the new boundary.
- **Minimum hold.** A stop whose `released_at` is less than 300 ms after `pressed_at` is a tap, and so is the hold machine's `DiscardTap` (`Session::discard_tap`). The take ends `cancelled` with reason `tap` before the speech gate and the engine, stores nothing, and `Feedback` resumes paused players without a stop cue. The 300 ms is `dettivo_hotkeys::hold::MIN_HOLD`, the same constant the hold machine uses, carried in `Policy.min_hold`.
- The tail and the minimum hold apply with the always-ready microphone off too.

## Consequences

- A sound in the 200 ms after the release is in the take and reaches the speech gate. A key-up click there can lift a silent take over the level stage. The voice stage still finds no speech in it when the Silero model is present (0081).
- `dictation.release` and `dictation.stop` answer at least 200 ms later than before, because they wait for the take to settle. `timings.capture_ms` now includes the tail.
- `dictation.stop` and `dictation.toggle` within 300 ms of the start answer `CONFLICT` with `kind = sessionCancelled`, as a cancelled take does. `dictation.release` on a tap answers `discarded`.
- Measured on the mock microphone through the daemon (`crates/dettivod/tests/take_window.rs`), a hold of H ms gives a take of H + 600 to H + 720 ms. The session tests pin the parts (`crates/dettivo-session/tests/take_window.rs`): speech that began 100 ms before the press starts 330 to 370 ms into the take, a release 150 ms before the last word ends keeps all 840 ms of the word, a 400 ms hold whose start ran a second late still gives 1,040 to 1,060 ms, and a 120 ms press reaches no engine.
- `Event::Pcm` carries its capture stamp (`at`). The meeting worker ignores it; a test builds a chunk delivered now with `Event::pcm`.
- A take is written once it ended instead of chunk by chunk. A dictation take was never recovered after a crash, so nothing that read the partial file is lost.
- A test that needs a sample-exact end sets `Policy.release_tail` to zero.

## Amendment 2026-11-06: a silent ending ends at the release

### What this gives you

A take you release after you stopped speaking no longer waits 200 ms for a word that is not coming. Its text arrives about 180 ms sooner: on the reference GPU a silent ending's release-to-text fell from 278 to 95 ms at p50, and `capture_ms` from 220 to 40 ms. A take released mid-word keeps the whole tail and is no slower.

### Situation

The tail made up about 200 of the 220 ms `capture_ms` in every take. Most releases come after the speech has ended. Of the 52 takes kept in one user's History, all recorded before the tail existed and so ending at the release, 46 had no speech in their last 150 ms by the gate's rules, 50 in their last 100 ms and 38 in their last 200 ms. Their last 150 ms peaked between -89 and -33 dBFS, mostly under -60, apart from two digitally silent endings, so the level stage settled 42 of the 46 and the voice check the other 4. Since ADR 0097 the Silero check has heard the take while it was captured, so at the release it can say where the voice last was without a pass over the take.

### Decision

- **The rule.** Once the release is seen and audio captured after it has arrived, the worker looks at the take's last 150 ms (`SILENT_ENDING`). That stretch is silent when its loudest 30 ms window, DC removed, is under `silence_dbfs` (-50 dBFS), or, when the level hears sound, when no Silero window there reached the gate's 0.4 threshold. A silent ending ends the take at once. Anything else keeps the 200 ms tail and `TAIL_STALL_GRACE` exactly as before: speech in the stretch, no streaming voice detector, a check that failed, or an answer that has not arrived when the tail ends.
- **Where the take ends.** At the first 512-sample Silero window boundary at or after both the release and what the check already holds, at most 32 ms past the release when the stop is seen promptly. The check has then run every sample the take keeps, so the gate's verdict needs no pass at the release, and the verdict is the one a pass over the take gives (ADR 0097). An early end never keeps audio past the tail's own cut.
- **Frame probabilities, not segments.** The voice test reads each window's probability against the gate's threshold. The gate's segments drop a stretch under 160 ms and close a segment only after 100 ms of quiet, so a word begun just before the release could count as no speech there; a single window at 0.4 keeps the tail instead.
- **Protocol.** `feed_speech` takes an optional `threshold` and then answers `voiced_until`, the end in samples of the last window run whose probability reached it, 0 when none did. The session's speech-check thread sends the take's samples up to the boundary with that question and the recording loop polls the answer, so the capture never waits on it.
- **Why 150 ms.** Within jfk's phrases the Silero probability never dips under the threshold for more than 44 ms, and a stop consonant's closure runs up to about 100 ms of near-silence, so 150 ms leaves a margin over both. 100 ms would end 4 more of the 52 History takes early with half the margin; 200 ms would keep the tail on 8 more. There is no further margin: the take ends at the release, so a word that ended before the release is kept whole.
- **No setting.** One rule serves speed and safety on every measured take, so none is added.

### Measurement

Release builds of `dettivod`, `dettivo`, `dettivo-engine-whisper` and `dettivo-engine-parakeet` with Vulkan, from `95a4f66` (before) and from this change (after), on 2026-11-06. Parakeet Ultra in mode `raw` with `whisper/silero-vad` on an RTX 4090 (Ryzen 9 7950X3D), each daemon in an isolated profile under `scripts/qa/xvfb-session.sh` with `DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_INSERT`. Release-to-text runs from sending `dictation.stop` to the `inserting` state event, as in ADR 0094. The builds alternated in blocks of 12 to 18 takes, because other processes loaded the GPU between 2 and 97% during the run. Silent endings stop jfk in its pauses (2.7, 4.9 and 7.9 s), where its floor is about -39 dBFS and the voice check decides; the level clip stops 0.4 s into digital silence after jfk's first phrase. Speech endings stop jfk mid-word (2.0, 6.5 and 9.5 s). [The benchmark report](../reports/benchmarks/release-tail-2026-11-06.md) lists every take.

| Takes | n per build | Release-to-text p50 (p10-p90), before | After | `capture_ms` p50, before and after |
| --- | --- | --- | --- | --- |
| Silent ending, voice check (jfk) | 24 | 278 (265-312) | 95 (84-146) | 220, 40 |
| Silent ending, level (digital silence) | 10 | 265 (265-302) | 86 (84-179) | 220, 40 |
| Speech ending (jfk) | 60 | 286 (264-383) | 283 (264-298) | 220, 220 |
| Speech ending (level clip) | 10 | 263 (263-312) | 264 (262-351) | 220, 220 |

Silent endings save 183 ms (voice check) and 179 ms (level) at p50. Speech endings keep `capture_ms` at 220-221 ms in all 70 takes of each build; the two p50s differ by -3 and +1 ms, inside the block-to-block spread of the before build, whose block p50s ran from 281 to 326 ms. The slowest takes of both builds are `transcribe_ms` spikes of 400-600 ms that came with the GPU load.

### Consequences

- The tail tests of this ADR keep their numbers: a word that ends inside the last 150 ms keeps the tail. `crates/dettivo-session/tests/silent_ending.rs` adds a silent ending under digital silence and under a hum, a word ending 60, 100 and 140 ms before the release, a check that cannot answer and a detector that only judges whole takes. `crates/dettivo-engine-whisper/tests/vad_voiced.rs` checks `voiced_until` on jfk mid-sentence, in its pause and on silence, and that asking leaves the gate's verdict unchanged.
- A key-up click within the 32 ms after the release can land in the judged stretch. It lifts the level and the voice check then decides, so it costs at most the tail.
- The voice process answers one more `feed_speech` per take, at the release, in parallel with the tail.
