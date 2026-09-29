# Re-run a failed meeting from the app

## Goal & Context

A meeting whose transcription failed (for example `transcription failed: chunk 193 of 262 failed: engine crashed`) keeps its audio, and `dettivo meetings recover <id>` transcribes it again through the daemon's `meetings.recover`. The meeting view offered no way to do the same: its Re-run button was always disabled, so the user had to leave the app for the CLI. This spec makes Re-run in the meeting view retry such a meeting.

While the recovered meeting ran its speaker pass, the processing strip's indeterminate progress sweep also drew outside the strip, as a teal line from the window's left edge across the sidebar. The same spec fixes that.

## Acceptance

- **R1:** Re-run in the meeting view is enabled for a `failed`, `partial`, `cancelled` or `stopped` meeting that the daemon lists under `meetings.status.recoverable` (its audio is still retained), and it calls `meetings.recover` for it, never `transcripts.rerun`.
- **R2:** Re-run stays disabled with a reason in its tooltip and accessible description when the audio is gone, while the meeting records or is being transcribed, and while the app is still checking the audio.
- **R3:** A completed meeting keeps its existing Re-run behaviour (disabled, with the reserved-in-the-contract reason).
- **R4:** Once `meetings.recover` answers, the meeting view reads the meeting again and shows the transcription running on the processing strip, as it does for any other run; a refusal reads under the header.
- **R5:** Tests cover the enable/disable rule (C++ detail model against the fake link) and the recover call from the header (QML test with the fake actions).
- **R6:** The indeterminate progress sweep (DettivoStyle `ProgressBar`) never draws outside its bar: the processing strip's bar under the running stage stays inside the strip. A QML test checks that no sweep pixel lands left of the bar across a full sweep cycle.

## Boundaries

- No daemon or protocol change: `meetings.recover` and `meetings.status.recoverable` already exist.
- No re-run of a completed meeting with another engine; that stays reserved.

## Decision Context

Reported by Gordon on 2026-09-29 after recovering a crashed meeting from the CLI and seeing the stray progress line on the recovered meeting.
