# 0092. Dictation can keep the microphone ready, and it is off by default

Status: Accepted 2026-11-02. Amends 0082 (where a take's audio comes from).

## What this gives you

With `[audio] keep_microphone_ready = true`, a take starts 450 ms before you pressed the key, so the first syllable you spoke as the key went down is in the take. A fresh install never opens the microphone while idle, so the desktop's microphone indicator stays dark until you dictate.

## Situation

- A capture that opens on the press loses what was said while PipeWire connected the stream. The only way to have audio from before the press is a capture that is already open.
- An open capture keeps the desktop's microphone-in-use indicator lit the whole time. That is a privacy cost the user has to choose.
- A meeting holds the microphone as its own capture. fn-65 will let a dictation run beside a meeting, so the rules for sharing the microphone have to hold before it lands.
- The daemon has no "dictation disabled" switch and no "pause Dettivo" verb. The setting itself is the only switch.

## Decision

- **The ring.** With the setting on, `MicReady` (`crates/dettivod/src/ready.rs`) keeps one 16 kHz mono capture open into an `AudioRing` (`crates/dettivo-audio/src/ring.rs`) that holds the last 10 s. Each chunk is stamped with `Instant` (`CLOCK_MONOTONIC`) when the daemon receives it, and the stamp is taken as the time of its last sample. `ReadyFeed::take(since)` (`crates/dettivo-session/src/ready.rs`) cuts the ring at the exact sample and then forwards the live chunks, so a take taps the ring's capture instead of opening a second one.
- **Default off.** `keep_microphone_ready` defaults to `false`. Off, no stream is open while idle and a take opens its own capture on the press, as before.
- **Privacy.** Ring audio stays in memory inside `dettivod`. No event, method or file carries it, and the ring overwrites itself continuously. Only the take window reaches the take file, under the existing `[history]` retention. Idle level samples are not published.
- **When it closes.** The daemon reconciles the ring every 250 ms under the capture reservation (`Daemon::capture_start_lock`). It closes when the setting is turned off, when logind reports the graphical session locked (`LockedHint`) or inactive (`Active = false`), and while a meeting captures. A take that taps the ring keeps it until the take stops. A dictation on its own capture keeps the ring from opening until it ends.
- **One capture of the microphone during a meeting.** `meetings.start` closes the ring inside its reservation, after its refusal checks and before the meeting opens its streams. `ReadyFeed::close` returns only after the capture acknowledged its stop. The ring reopens on the first tick after the meeting's capture ended, that is once the meeting session leaves `recording` and `stopping` and its worker has stopped both tracks. A press during the reopen gets the pre-roll the ring has collected so far.
- **Device changes.** The ring follows the same default-source rule as the meeting microphone (0027). A moved default clears the ring and reopens the capture on the next tick. A capture that ended (a lost pinned device, no source) reopens the same way. A capture that cannot open sets `audio.devices.microphone_ready` to `failed` with the reason, `dettivo doctor` shows it, and the daemon tries again every 2 s. Dictation then opens the microphone on press, so the failure never fails a dictation.
- **Rules fn-65 builds on.** Dictation during a meeting stays refused with `sessionActive` until fn-65 lands. When it does:
  - The meeting's microphone stream feeds an in-memory tap with the `AudioRing` contract. Pre-roll during a meeting comes from that tap whatever the setting says, because the microphone is already open and no new stream or indicator is involved. The meeting's takes and journal are untouched by the tap.
  - The span the meeting marks and silences under `meetings.dictation_overlap = "exclude"` is the take window from 0093: from the press minus 450 ms to the end of the release tail. A span cut at the press and the release would leak the dictation's first and last words into the meeting transcript.
  - A tap under 300 ms marks no span and silences nothing in the meeting.
  - The release tail and the minimum hold count as dictation time, so a meeting start during either is refused with `sessionActive`.

## Consequences

- Turning the setting on lights the microphone indicator while the daemon runs, the session is unlocked and no meeting records. `docs/dictation.md` says so where it describes the setting.
- A chunk's stamp trails the microphone by the capture's delivery latency, at most one PipeWire quantum (about 21 ms at the default 1024 frames at 48 kHz). The pre-roll therefore reaches slightly further back than 450 ms, never less.
- On a desktop whose locker does not set logind's `LockedHint` (Omarchy's Quickshell lock, docs/qa.md), the ring stays open while the screen is locked. Without logind the ring follows the setting alone and logs that once.
- There is no pause verb, so "pause" from the spec has nothing to attach to. A pause verb, when one lands, adds one reason to `closed_because` in `ready.rs`.
- The meeting handoff relies on the audio thread running commands in order: the ring's `Cmd::Stop` runs before the meeting's `Cmd::Open`. The mock microphone cannot show PipeWire's capture list, so the tests count open captures through a fake opener (`crates/dettivod/src/ready_tests.rs`) and check the log order through the real `meetings.start` (`crates/dettivod/tests/take_window.rs`).
