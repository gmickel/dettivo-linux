# Dictation decodes while the key is held

## Goal & Context

Release-to-text grows with take length: 489 ms p50 at 30 s, 583 ms at 60 s, 890 ms at 120 s on the RTX 4090 with Vulkan and Parakeet (ADR 0097). Most of the growth is the offline decode over the whole take after release. ADR 0094 measured that parakeet.cpp cannot stream the v2/v3/Ultra dictation models, and rejected a buffered decoder with 5.6 s of left context (Option A).

An offline model can still do most of its work during the hold without streaming. While the key is held, re-transcribe the recording so far at a fixed cadence; once the take is long, commit the words that lie well behind the newest audio while keeping a long left context (on the order of 20 s); at release, decode only the uncommitted tail (on the order of the last 8 s plus 2 s of overlap) and stitch by word timestamps. Published measurements of this scheme on Parakeet TDT found WER unchanged against a whole-file decode on 1,351 public clips (+0.0009, 95% CI [-0.0006, +0.0023]), while a short 4 s left context cost +0.0119. That is why short windows, ADR 0094's Option A and the meeting path's 3 s windows (ADR 0031), are the wrong shape for dictation. If only silence followed the last round, that round's text is the answer and nothing is decoded at release.

This spec brings that scheme to Dettivo, so release-to-text stops growing with take length without a streaming model and without losing accuracy.

Origin: Gordon, 2026-11-06.

## Approach

- During a held take, decode the captured audio in rounds at a cadence chosen from measurement (1-2 s on the reference GPU is the starting point; a 60 s v3 window costs 280 ms, ADR 0094). Each round re-decodes from the last committed point with word timestamps.
- Commit words older than a left-context horizon (20 s once the take passes 60 s is the starting point); choose the horizon from Dettivo's own WER measurement.
- At release, decode only the uncommitted tail (8 s plus 2 s overlap is the starting point) and stitch by word timestamps. When the audio after the last round is silent by the speech check, reuse that round's text.
- Rounds yield to a meeting and to the release decode, and capture is never delayed. A round that cannot keep up is skipped, and the release falls back to today's whole-take decode, recorded in History.
- Parakeet first. Whisper (the default engine, ADR 0085) is measured with the same scheme and ships only if R2's bound holds for it too.
- A new ADR amends ADR 0094.

## Acceptance

- **R1:** On the reference GPU, release-to-text p50 through the daemon (at least 5 runs each) for 30, 60 and 120 s Parakeet takes is at most 300 ms, and p90 does not exceed today's p50 (489 / 583 / 890 ms).
- **R2:** Windowed WER minus whole-take WER, by paired bootstrap on the existing WER fixtures, has an upper 95% CI bound of at most +0.004 for Parakeet v3 and Ultra, in English and German.
- **R3:** A take with no rounds (too short, or every round skipped) produces text byte-identical to today's.
- **R4:** Capture never waits on a round and no sample is dropped: a test feeds audio faster than rounds complete and asserts every sample reaches the take.
- **R5:** Whisper is measured with the same scheme; the ADR records whether it ships for Whisper and why.
- **R6:** The ADR records the cadence, horizon, release window, GPU busy share during a hold, and before/after numbers; `docs/dictation.md` and `docs/engines.md` follow; a changelog entry is added.

## Boundaries

- No streaming model and no change of dictation model (ADR 0094).
- The speech gate (fn-79, fn-90) and the vocabulary paths (fn-84, fn-89) keep their behaviour; vocabulary decisions run on the final stitched text.
- CPU-only machines keep today's whole-take decode unless measurement shows rounds help there.
- The always-ready microphone and meeting rules (fn-81) are unchanged.

## Quick commands

- `cargo test -p dettivo-transcribe -p dettivo-session -p dettivo-engine-parakeet -p dettivod`
- `dettivo-qa bench`
- `just build test lint`
