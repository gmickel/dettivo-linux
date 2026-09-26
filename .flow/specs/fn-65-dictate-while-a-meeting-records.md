# Dictate while a meeting records

## Goal & Context

Today the daemon allows one recording at a time. A dictation hotkey pressed during a meeting is refused with `CONFLICT` / `sessionActive`, and the only trace is a warning in the journal: no cue plays, the pill doesn't change, and dictation looks broken (`crates/dettivod/src/actions.rs:108-117`, `:282-296`). People want to dictate a chat reply, a note or an email while they sit in a call that Dettivo is recording.

This spec lets a dictation run while a meeting is capturing. The meeting keeps recording without a gap, the dictated speech stays out of the meeting's transcript, and dictation works exactly as it does outside a meeting.

The call itself is the user's responsibility: they mute themselves in Teams, Zoom or whichever call app they use. Dettivo never integrates with call apps. We assume headphones, which almost everyone uses in calls, so there is no echo handling.

## Approach

- Session model: at most one meeting and one dictation at a time. A dictation may start while a meeting is capturing. A meeting start during a dictation stays refused. The existing `capture_start_lock()` still serialises starts (ADR 0046).
- Microphone: dictation opens its own PipeWire capture stream, as it does today. The meeting's mic and system streams keep running untouched.
- Meeting recording: by default, the meeting's mic track is written as silence for the dictation span, and the span is recorded in the meeting's metadata as a dictation span. Transcription, diarization and analysis treat it as a gap. The system track is not touched.
- Engines: this spec covers the case where dictation and the meeting run in separate engine processes. That is the default setup: Parakeet dictation and a Whisper meeting model (ADR 0060). When both would use the same engine process, dictation stays refused, now with a visible notice. A priority lane for that case is a later spec.
- Feedback: dictation shows its usual pill state with a small recording indicator for the meeting. Sound cues stay silent while a meeting is capturing, because the system-audio capture would record them. Wire the stale `meeting_active()` stub in `feedback.rs` to real meeting state.

## Quick commands

- `just build test lint`

## Acceptance

- **R1:** While a meeting is capturing, a dictation started by hotkey, CLI or RPC runs and inserts its text as it does outside a meeting, when the dictation engine and the meeting engine are separate processes.
- **R2:** The meeting capture continues without interruption through the dictation: no dropped takes or gaps in either track, and the meeting's clock and state are unaffected.
- **R3:** With the default setting, the meeting mic track is silence for the dictation span, and the span is stored in the meeting's metadata. The meeting's transcript, speaker pass and analysis contain none of the dictated speech.
- **R4:** `meetings.dictation_overlap` in `config.toml` takes `"exclude"` (the default, R3) or `"keep"` (the dictated speech stays in the meeting recording). It is documented in `docs/config.md`.
- **R5:** A meeting start while a dictation runs is refused with `sessionActive`, as today. A second dictation is refused as today.
- **R6:** When dictation is refused during a meeting (a shared engine process, or any other refusal), the pill shows a short visible notice saying why, instead of failing silently.
- **R7:** No sound cues play while a meeting is capturing, including for dictations. The pill shows the dictation state with the meeting still visibly recording.
- **R8:** Daemon tests cover: a dictation during a meeting (R1, R2), span marking and exclusion (R3), the `keep` setting (R4), the refusals (R5, R6), and the start race of ADR 0046 with the new rule.
- **R9:** An ADR records the change from "one recording at a time" (ADR 0063 and the related docs) to "one meeting plus one dictation". `docs/dictation.md`, `docs/meetings.md` and `docs/hotkeys.md` say that you mute yourself in your call app and use headphones.

## Boundaries

- No echo cancellation or echo guard. Headphones are assumed.
- No integration with call apps: Dettivo never mutes or controls Teams, Zoom or others.
- No priority lane or per-model engine processes for the shared-engine case. Dictation there stays refused, with the R6 notice.
- No change to meeting finalisation, which already allows dictation once capture ends.

## Decision Context

The design came up on 2026-09-25 while Gordon was recording a meeting. The user owns muting in the call app, and headphones are assumed (99% of users), so echo handling is YAGNI. It is added only if someone reports remote voices in their dictation. The visible notice alone would matter only until this spec lands, so it is folded in as R6 for the shared-engine case, not split into its own spec.
