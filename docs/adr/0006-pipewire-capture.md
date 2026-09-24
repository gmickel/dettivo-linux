# 0006. PipeWire captures microphone and system audio and resamples on the way in

Status: Accepted 2026-09-03

## What this gives you

Meetings capture both your microphone and what the other side says, follow your default devices when you plug in a headset mid-call, and keep both tracks on one clock so the merger can interleave them by time.

## Situation

Voxtype uses CPAL, which reaches ALSA through PipeWire's compatibility layer and cannot express a capture stream on a sink monitor, follow default-device changes, or share a clock between two streams. PipeWire 1.6 is the audio server on every current desktop, and its native API accepts a requested format (16 kHz mono) and resamples inside the graph.

## Decision

`dettivo-audio` uses the `pipewire` crate (pipewire-rs 0.10.1, maintained by the PipeWire project). Microphone capture follows the default source unless a device is pinned. System audio is a capture stream on the monitor of the default sink, re-targeted when the metadata `default.audio.sink` changes. Both streams request 16 kHz mono so no resampler runs in the daemon.

## Consequences

- A device switch during dictation ends the session with a recoverable error; during a meeting the daemon restarts the microphone stream, keeps each take as a separate file with its offset, and marks the gap.
- The virtual audio rig for QA is the same mechanism in reverse: null sinks created with `pactl`, fixtures played with `pw-play`, monitors used as inputs.
- Hosts without PipeWire are unsupported in v1; an `AudioSource` trait keeps CPAL addable later.
- Echo between a user's own voice on the microphone and on the sink monitor is a known limit with speakers; headsets avoid it and analysis notes it.
