# Word timings from Whisper for speaker labelling

## Goal & Context

fn-68 gives nearly every remote line a speaker by sentence. On AMI dev, most lines the old rule left blank hold two speakers, often a short reply inside someone else's turn. Without word timings a sentence can take only one speaker, so those lines are right only about half the time with the current engine and two-thirds with Nemotron. Labelled lines with the wrong speaker rose 5 points (fn-68, ADR 0072). Gordon accepted that trade on 2026-09-26 on condition that word timings follow.

Our Whisper engine discards word timing: every segment it returns has `words: Vec::new()` (`crates/dettivo-engine-whisper/src/engine.rs`). whisper.cpp can time each token, and whisper-rs 0.16 exposes both plain token timestamps and DTW token timestamps (`DtwParameters`, `DtwModelPreset` for the Whisper model family), which align more precisely. With words, fn-68's rule already cuts a sentence at a pause where the diarized speaker changes (`pause_ms`), so a two-speaker line splits at the change instead of being guessed.

This spec makes Whisper return timed words in meetings and imports, and measures what that does to speaker labelling.

## Approach

- The Whisper engine fills `Segment.words` (text, start, end, and confidence where available) when timestamps are asked for. Use DTW token timestamps with the preset for the loaded model where one exists, otherwise plain token timestamps. Merge sub-word tokens into words.
- Measure word timing accuracy against the AMI reference word times the fn-67 bench already has, and against the fixture golden `crates/dettivo-qa/fixtures/alignment/jfk.words.json` (ADR 0018's protocol). Choose DTW or plain from the numbers.
- Record the cost: extra time per meeting hour on CPU and Vulkan, and any change in word error rate (none is expected).
- The bench's ASR stage caches words. Re-run the bench with fn-68's rule and tune `pause_ms` on AMI dev.
- Meetings, imports and re-runs store the words (the contract `Segment.words` already exists). Dictation is unchanged unless the words are free.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** Whisper meeting and import transcripts carry timed words on every segment. Word start and end offsets on AMI dev have a reported median and p95 against the reference, and so does the jfk fixture against its golden.
- **R2:** On the fn-67 bench with fn-68's rule and words, labelled lines with the wrong speaker on AMI dev return to within 1 point of the old rule's figure (15.4% current engine, 5.5% Nemotron), while blank remote lines on the local meetings stay under 3%. If that isn't reached, the report says how far it got and why.
- **R3:** The headline attribution error on AMI dev falls further than fn-68's 22.4% (current engine) and 14.7% (Nemotron), and the `--heldout` AMI test figures confirm it.
- **R4:** Word error rate on the existing Whisper fixtures is unchanged, and the added time per meeting hour on CPU and Vulkan is reported.
- **R5:** An ADR records the timing method (DTW or plain), its measured accuracy and cost, and amends ADR 0072's note on word timings. `docs/meetings.md` and `docs/engines.md` say meetings carry word timings.

## Boundaries

- No change to the assignment rule's design beyond tuning `pause_ms` (fn-68 owns the rule).
- Parakeet stays dictation-only (ADR 0018).
- No voice check here (fn-70).

## Decision Context

Gordon chose fn-68's label-everything default on 2026-09-26 with word timings as the immediate follow-up, to bring the wrong-name rate back down.

### R2 outcome (2026-09-27)

R2 is missed: with word timings, labelled lines with the wrong speaker on AMI dev are 21.02% (current engine) and 11.24% (Nemotron), against targets of 15.4% and 5.5%. A pause cut needs the diarized speaker change within `pause_ms` of the word gap, and the engines place changes a few hundred milliseconds off. R3 is met: attribution error falls to 21.07% and 12.49% on AMI dev, and to 14.11% and 11.96% on the held-out test. Gordon chose to merge on 2026-09-27 and continue hill-climbing, with the voice check (fn-70) and turn-boundary work targeting the wrong names.
