# Two-track speaker rules: bleed, single remote, shared mic, your own voice

## Goal & Context

Dettivo records the microphone ("You") and the system track (the remote side), and diarizes only the system track. The research confirms that is the right design. The best open recorders add four rules on top, which Dettivo lacks or has only partly.

1. **Bleed.** Remote audio played through speakers leaks into the mic and shows up as "You". Dettivo drops mic filler near long remote segments (`cross_source_padding_ms`). omarchy-meeting-recorder keeps a mic frame only when it is at least half as loud as nearby system audio, and drops "You" sentences whose word trigrams repeat remote text. OpenWhispr drops mic segments that match system text within 6 s.
2. **Single remote participant.** With exactly one remote voice, anarlog labels the system track directly without diarizing, which removes all remote confusion.
3. **Shared mic.** When the system track is silent while several people speak, the mic is a room mic. anarlog then diarizes the mic, and omarchy-meeting-recorder v1.2 labels "You 1/2". Dettivo has a room-audio mode.
4. **Your own voice.** Every meeting's mic track is free enrolment audio for the user's voiceprint. It can catch the user's voice wrongly counted on the system track, and identify the user in room audio.

Details are in the vault note "dettivo-linux -- Speaker attribution research (2026-09-26)".

## Approach

- Measure each rule on the fn-67 bench (the local/remote proxy, leaked "You" lines, the headline number), and keep only the rules that help.
- The bleed gate uses levels and text overlap. The research lists echo cancellation (WebRTC AEC3 via the pure-Rust `sonora` crate) as the heavier option: headphones are the norm (fn-65), so try the cheap gate first and add cancellation only if the bench still shows leaks.
- The single-remote shortcut triggers when the engine and the voice check agree there is one remote speaker. It is a labelling choice, and never changes the audio.
- The user's voiceprint is stored locally, is opt-out in `config.toml`, and never leaves the machine.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** On the fn-67 bench's local meetings, leaked remote speech labelled "You" falls. The bench reports it through the local/remote proxy, and each rule's contribution is reported separately.
- **R2:** Meetings with a single remote participant have zero remote speaker confusion, and a test covers the shortcut.
- **R3:** Shared-mic detection switches to room-audio labelling when its condition holds. A test with a fixture covers it, and a false switch on a normal two-track meeting is covered too.
- **R4:** The user's voiceprint is built from the mic track, stored locally with a documented opt-out, and used to relabel system-track segments that match the user. The bench reports the effect.
- **R5:** No rule makes the headline attribution error worse on AMI dev or on the local meetings.
- **R6:** `docs/meetings.md` and `docs/config.md` document the rules and their keys. An ADR records them.

## Boundaries

- No echo cancellation unless the cheap gate fails R1. If it fails, record that in the ADR and capture a follow-up spec.
- Nothing is sent off the machine, including voiceprints.
- No LLM correction.

## Decision Context

Gordon approved recommendation 4 on 2026-09-26.
