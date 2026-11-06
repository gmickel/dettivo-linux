# The release tail ends early on a silent ending

## Goal & Context

Every take waits 200 ms after the key is released (`POST_RELEASE_TAIL`, `crates/dettivo-session/src/lib.rs:115`, ADR 0093), so the last word is not clipped. That tail is about 200 of the ~220 ms `capture_ms` in every release-to-text figure ADR 0097 records (489 ms p50 at 30 s, 583 ms at 60 s, 890 ms at 120 s on the RTX 4090). Published dictation measurements put the median pause before letting go of the key at about 0.24 s, with roughly a third of releases already ending in silence. For those takes the tail captures nothing a word needs.

Since ADR 0097 the Silero speech check runs while the take is captured, so at release Dettivo already knows whether the last stretch of audio was speech. This spec ends the tail at once when it was not, so a large share of takes become up to 200 ms faster with no cost to the rest.

Origin: Gordon, 2026-11-06.

## Approach

- At release, read the incremental speech check's verdict for the audio just before the release. If the last window (about 100-200 ms, chosen from measurement) holds no speech by the gate's own rules (fn-79: -50 dBFS loudest 30 ms window, Silero 0.4), end the take at release instead of after the tail.
- Otherwise keep the 200 ms tail exactly as today, including `TAIL_STALL_GRACE`.
- Measure the share of silent endings on History-like takes, choose the silence window and any margin from them and the existing tail fixtures, and record the margin.

## Acceptance

- **R1:** A take whose final window is silent ends at release; a take whose final window holds speech keeps the full 200 ms tail. Tests cover both, and fn-79's and fn-90's gate tests pass unmodified.
- **R2:** No last word is clipped: the existing release-tail fixtures and a new fixture with a word ending 50-150 ms before release keep their final word.
- **R3:** On the reference GPU, `capture_ms` and release-to-text are reported as distributions over at least 20 takes with silent endings and 20 with speech endings, through the daemon. Silent-ending takes save at least 150 ms p50; speech-ending takes are no slower.
- **R4:** ADR 0093 is amended, or a new ADR supersedes its tail rule, with the measurement and margin; `docs/dictation.md` follows; a changelog entry is added.

## Boundaries

- The gate's thresholds and verdicts do not change.
- No new user setting unless the measurement shows one rule cannot serve both speed and safety.
- Tests and QA use fixture audio, never the live microphone.

## Quick commands

- `cargo test -p dettivo-session -p dettivo-speech -p dettivod`
- `dettivo-qa bench`
- `just build test lint`
