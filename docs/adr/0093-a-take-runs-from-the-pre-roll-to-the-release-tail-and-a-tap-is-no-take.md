# 0093. A take runs from the pre-roll to the release tail, and a tap is no take

Status: Accepted 2026-11-02. Amends 0082 (where a take ends) and 0083 (how a tap ends).

## What this gives you

A last word still sounding when you let go of the key is in the take, because recording continues for 200 ms after the release. A key you only tapped never reaches the speech engine, plays no stop cue and leaves nothing in History.

## Situation

- 0082 ended the capture at the stop request, on the stopping thread. That keeps a key-up click out of the take, and it also cuts the end of a word the user released the key on.
- 0083's hold machine already drops a take released under 300 ms, through `dictation.cancel`'s path. That take ended `cancelled` with reason `cancelled` and played the stop cue.
- The spec asks for `press_time` to come from a `--pressed-at` monotonic timestamp the CLI fills in. fn-82 already times every edge by when it reached the daemon (`dictation.press` and `dictation.release` take `Instant::now()` before they parse their parameters), and a press that queues behind a slow start keeps its arrival time.

## Decision

- **The press time is the arrival time.** `dictation.press`, `dictation.release`, `dictation.start`, `dictation.stop`, `dictation.toggle` and the portal and evdev edges pass the `Instant` the call reached the daemon. `Session::start` takes it as `pressed_at` and `Snapshot.started` carries it. No `--pressed-at` flag exists; the CLI's process start (a few milliseconds) is the gap left between the key and the press time.
- **Pre-roll.** With the always-ready microphone open (0092), the take starts at `pressed_at − 450 ms` (`PRE_ROLL`). With it off, the take starts with the first chunk the new capture delivers.
- **Release tail.** `Session::stop_expected(job, released_at)` records the release time and the worker keeps recording. It stops the source once the source delivered audio captured at or after `released_at + 200 ms` (`POST_RELEASE_TAIL`), or once no audio arrived for 100 ms (`TAIL_STALL_GRACE`), so a stalled device closes the take after the grace.
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
