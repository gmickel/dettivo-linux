# 0082. A dictation take ends at the stop request, within one capture quantum

Status: Accepted 2026-11-01. Amends 0081 (what the level gate leaves out).

## What this gives you

A sound you make after releasing the dictation key, the next keystroke or a door, never decides whether the take is speech. The gate judges what you recorded up to the release, plus at most one capture quantum (about 21 ms) that PipeWire may already have had in flight.

## Situation

fn-79's R1 asks the level gate to leave out the post-release tail, so that a stop cue cannot count as speech. Two ways to meet it were on the table: mark the release inside the take and gate only the samples before the mark, or end the capture at the stop request and gate the whole take.

The mark needs the session worker to tell queued audio from audio recorded after the release. It cannot tell them apart: a chunk that arrives while the worker waits may have been recorded before or after the release, so every attempt moved the error from one side of the mark to the other.

Ending the capture at the request leaves a small, bounded race. `Session::stop_expected` calls the source's stopper on the stopping thread. For a PipeWire capture that sends `Cmd::Stop` to the `dettivo-audio` thread, which disconnects the stream under the loop lock, and from then on `CaptureData::publish` sends nothing. Until `Cmd::Stop` runs, PipeWire's process callback keeps publishing, so the quantum it was filling when the request was sent can still reach the take. At PipeWire's default quantum of 1024 frames at 48 kHz that is about 21 ms.

## Decision

- The capture ends at the stop request. The stopping thread ends the source before the session worker reacts, and the audio layer publishes no samples after a capture's end. The level gate judges the whole take.
- Up to one PipeWire capture quantum delivered after the stop request, but before `Cmd::Stop` runs, counts as inside the take for R1. The gate accepts it and does not try to trim it.
- The mock microphone (`DETTIVO_MOCK_MIC`) keeps the stricter bound: it checks the stop after its real-time wait, so a stop during the wait publishes no chunk at all.
- Gordon made this call on 2026-11-01 (option (a) of the R1 question during fn-79's implementation review).

## Consequences

- The stop cue cannot reach the take. `dettivod`'s feedback (`effect_of`) plays it when the session reaches `idle` or `cancelled`. The worker publishes those only after the capture has ended and the gate has judged the take.
- A sound in the one quantum after the release can still count. To lift a silent take over -50 dBFS on its own it has to put that energy into one 30 ms window, so a click right at the release may pass the level stage. The voice stage still finds no speech in it when the Silero model is present.
- A test that needs a sample-exact release uses the mock microphone or a channel source, never PipeWire.
