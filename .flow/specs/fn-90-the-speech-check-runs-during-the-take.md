# The speech check runs during the take

## Goal & Context

Release-to-text is what a dictation user feels. fn-86 measured it on the reference GPU (RTX 4090, Vulkan, Parakeet): 594-657 ms for a 30 s take, 683 ms p50 at 60 s, 1,157 ms p50 at 120 s. Streaming is not needed to stay under 1 s up to about 90 s, and parakeet.cpp cannot stream the dictation models (ADR 0094). But one serial step sits in every take's release path: fn-79's Silero speech check runs over the whole take after release, before recognition starts. At 120 s it costs 336 ms, more than the 80-230 ms by which those takes miss 1 s.

fn-86 also found that 7 of 93 recorded Parakeet takes stalled for 1.1-3.9 s; three were the first take after the engine sat idle, four have no recorded cause.

This spec takes the speech check off the release path and removes the stalls a user can hit, so every take is faster and two-minute takes land under 1 s.

Origin: Gordon, 2026-11-02, goal fn-88 ("Close + VAD during take"), under his standing bar of best-in-class performance.

## Approach

- **Incremental speech check.** Run the level window and the Silero VAD over the audio as it is captured, in windows, on a worker that never blocks capture. At release only the last window (plus the release tail fn-81 adds) remains to judge, so the gate's verdict is ready within a few tens of milliseconds whatever the take's length. The verdict and its parameters stay exactly fn-79's (−50 dBFS loudest 30 ms window, Silero threshold 0.4, minimum speech 160 ms); only when it is computed changes. A take whose verdict is "speech" starts recognition immediately; the existing silence handling is unchanged.
- **Stalls.** Reproduce the first-take-after-idle stall (idle unload, then a take) and find where the time goes (engine spawn, model load, warm-up, GPU wake). Fix what a user can hit, for example by preloading on the press of a hold key when the engine was unloaded, or by keeping the engine loaded across short idle gaps, choosing from measurement. Investigate the four unexplained stalls from history timings (read-only) and fix any cause found; record what was not found.

## Acceptance

- **R1:** The speech gate's verdicts are unchanged: fn-79's gate tests and fixtures pass unmodified, and a test proves the incremental verdict equals the whole-take verdict on every gate fixture.
- **R2:** On the reference GPU, a 120 s Parakeet take reaches release-to-text in under 1 s at p50 through the daemon (QA rig, at least 5 runs), and 30 s and 60 s takes are at least as fast as today (594-657 ms and 683 ms p50). The numbers are recorded in a benchmark report.
- **R3:** The speech check never delays capture or drops audio: a test feeds a take faster than the check runs and asserts every sample reaches the take.
- **R4:** The first take after the engine was idle-unloaded reaches release-to-text within 20% of a warm take on the reference GPU for takes of 3 s or longer (the press-to-release time counts as available load time), or the ADR records why not with measurements.
- **R5:** An ADR records the incremental gate, the stall investigation (causes found and fixed, causes not found) and the before/after numbers. `docs/dictation.md` and `docs/engines.md` follow; a changelog entry is added.

## Boundaries

- The gate's thresholds and fn-79's acceptance criteria do not change.
- No streaming recognition (ADR 0094).
- CPU behaviour must not regress; fn-80's rule that CPU loads need no warm-up holds.

## Quick commands

- `cargo test -p dettivo-speech -p dettivo-session -p dettivod`
- `dettivo-qa bench`
- `just build test lint`
