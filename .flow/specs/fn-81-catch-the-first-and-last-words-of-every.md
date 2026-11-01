# Catch the first and last words of every take

## Goal & Context

People start speaking as they press the key, and release it while the last word is still sounding. Today capture opens on `dettivo dictation start` and stops on `stop`. The first syllable is lost to the time the stream takes to open, and the last one to an early release. A tap that was never meant as a dictation still runs the engine.

This spec records from slightly before the press to slightly after the release, and ignores taps.

## Approach

**Always-ready capture with a ring buffer (opt-in).**
- With `[audio] keep_microphone_ready = true`, while dictation is enabled, the graphical session is active and no meeting is capturing, keep a 16 kHz mono PipeWire capture open into a 10 s ring buffer.
- Each chunk carries a monotonic timestamp (`CLOCK_MONOTONIC`).
- `dictation.start` copies the audio from `press_time − 450 ms` onward out of the ring and then keeps appending live chunks.
- `press_time` is the moment the compositor binding fired, as reported by the CLI call (a `--pressed-at` monotonic timestamp the CLI fills in), not the moment the daemon processed it. That removes the key-to-audio delay as well.

```rust
pub struct AudioRing { buf: VecDeque<Chunk>, cap: Duration /* 10 s */ }
pub struct Chunk { at: MonoInstant, pcm: Box<[i16]> }

impl AudioRing {
    /// Every sample from `since` onward; the take continues live from here.
    fn since(&self, since: MonoInstant) -> Vec<i16> { /* trim first chunk to the exact sample */ }
}
const PRE_ROLL: Duration = Duration::from_millis(450);
```

**Release tail.** `dictation.stop` keeps recording for 200 ms after `released_at`, then closes the take. If audio stops arriving during the tail (a device stall), it closes after a 100 ms grace.

**Minimum hold.** A take whose press-to-release span is under 300 ms is discarded before the engine. There's no stop cue for it, it isn't stored in history, and it ends `cancelled` with reason `tap`.

```rust
const POST_RELEASE_TAIL: Duration = Duration::from_millis(200);
const MIN_HOLD: Duration = Duration::from_millis(300);
```

**Privacy and the mic indicator.** An always-open stream keeps the desktop's microphone-in-use indicator lit.
- Gate it behind `[audio] keep_microphone_ready`, **default `false`**. A fresh install never lights the indicator while idle. People who lose first words switch it on; `docs/dictation.md` says what it costs.
- When it is off, capture opens on press as today, and the pre-roll is whatever the stream delivers.
- Ring audio is never written to disk, never leaves the daemon, and is overwritten continuously. Only the take window is persisted, under the existing retention rules.
- The ring closes when dictation is disabled, the session locks, or the user pauses Dettivo.

**Meetings.** A meeting already holds the microphone open as its own capture, and fn-65 (dictate while a meeting records, not yet built) will let a dictation run beside it. The rules below keep the two from fighting over the source or leaking into each other.

- **One capture of the microphone.** When a meeting starts capturing, the ring closes before the meeting opens its streams, inside the same `capture_start_lock()` reservation, so the source is never captured twice. When the meeting's capture ends (it enters `stopping`), the ring reopens if the setting is on. A press during the reopen gets whatever pre-roll exists, as with the setting off.
- **Today, dictation during a meeting stays refused** (`sessionActive`), so the closed ring loses nothing.
- **Once fn-65 lets dictation run during a meeting:**
  - The meeting's microphone stream feeds an in-memory tap with the same `AudioRing` contract. Pre-roll during a meeting comes from that tap, whatever the setting says, because the microphone is already open and no new stream or indicator is involved. The meeting's takes and journal are untouched by the tap.
  - The dictation span the meeting marks and silences under `meetings.dictation_overlap = "exclude"` is the take window: from `press_time − 450 ms` to the end of the release tail. A span cut at the press and release instead would leak the dictation's first and last words into the meeting transcript.
  - A tap under 300 ms marks no span and silences nothing in the meeting.
  - The tail and the minimum hold count as dictation time, so a meeting start during either is refused with `sessionActive`, as during any dictation.
- **Device changes.** The ring follows the same default-source rule as the meeting's microphone (ADR 0027): a moved default reopens on the new one.

```rust
enum RingFeed { Own(CaptureHandle), MeetingTap(TapHandle), Closed }   // Own only when the setting is on and no meeting captures

fn on_meeting_capture(started: bool, cfg: &AudioConfig, ring: &mut Ring) {
    ring.feed = match (started, cfg.keep_microphone_ready) {
        (true, _) => RingFeed::Closed,            // the meeting opens its own streams next; fn-65 swaps in MeetingTap
        (false, true) => RingFeed::Own(open_capture(cfg)),
        (false, false) => RingFeed::Closed,
    };
}
```

## Acceptance

- **R1:** With the ring enabled, a take contains audio from 450 ms before the binding fired. A fixture that starts speaking 100 ms before the press keeps its first word.
- **R2:** A take contains 200 ms after the release. A fixture that releases 150 ms before the last word ends keeps that word.
- **R3:** A press shorter than 300 ms never reaches the engine, plays no stop cue, creates no history item, and ends `cancelled` with reason `tap`.
- **R4:** `[audio] keep_microphone_ready` controls the always-open stream and defaults to `false`. With it off, no stream is open while idle, and behaviour matches today's with the tail and minimum hold still applied.
- **R4a:** With the setting on, starting a meeting closes the ring before the meeting's streams open, and the ring reopens when the meeting enters `stopping`. At no point does PipeWire show two Dettivo captures of the microphone. A daemon test covers the handoff under the capture reservation.
- **R5:** Ring audio never touches disk or any client. A test asserts that only the take window is written, and the ring closes on disable, lock and pause.
- **R6:** A default source change or device loss while idle reopens the ring on the new default without user action. A failure is reported in `dettivo doctor`, not as a failed dictation.
- **R7:** An ADR records the default-off decision, the privacy rules and the meeting rules above, including the take-window span fn-65 must silence. `docs/dictation.md` and `docs/config.md` describe the pre-roll, the tail and the minimum hold.

## Boundaries

- No change to meeting capture, its takes or its journal. Dictation during a meeting stays refused until fn-65 lands; this spec only fixes the rules fn-65 builds on.
- The ring is dictation's only open stream while idle. No second capture of the same source, ever, including during a meeting.

## Quick commands

- `cargo test -p dettivo-audio -p dettivo-session`
- `just build test lint`
