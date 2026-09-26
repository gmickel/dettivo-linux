# Label every remote line with sentence-unit speaker voting

## Goal & Context

Today the speaker pass labels a transcript segment only when at least 25% of it lies inside diarized speech and one speaker holds at least 60% of that. The rest stay blank (`crates/dettivo-meeting/src/diarize.rs`, `winner`; `[meetings.diarization] min_coverage` and `min_speaker_share`). On retained meetings that leaves 22.7% of remote lines without a speaker (fn-64 report). None of the 13 open-source tools surveyed abstains like this; they label every line, or fall back to the nearest turn within a tolerance.

This spec replaces the rule with the approach the best prior art uses. Split the text into sentence- or pause-bounded units, give each unit the speaker who holds most of it, and fill units with no overlap from the nearest turn within a tolerance. The rule is engine-agnostic, so it improves both the current engine and Nemotron.

Prior art: omarchy-meeting-recorder `src/transcribe.rs` (a sentence is never split between speakers; cuts at sentence ends and at pauses of 250 ms or more only where the speaker changes; snaps line starts to the turn start within 1.5 s). Vibe (nearest turn only within 1.5 s). whisper-diarization (a sentence's majority speaker when it holds at least half its words). anarlog (merges micro-segments under 3 words and 1.5 s). The noScribe PR #351 finding: per-word assignment and cutting at every diarized change were net-harmful. Details are in the vault note "dettivo-linux -- Speaker attribution research (2026-09-26)".

## Approach

- Units come from Whisper's segments and word timestamps: cut at sentence ends, and at pauses above a threshold only where the diarized speaker differs across the pause. Very short fragments merge into a neighbour.
- Each unit takes the speaker with the most overlapping time. A unit with no overlap takes the nearest turn within a tolerance, and otherwise stays unlabelled. An unlabelled line is the rare exception, not a quarter of the transcript.
- A split unit becomes separate stored segments with their own times, so the transcript, search and exports keep working unchanged.
- Parameters are `config.toml` keys under `[meetings.diarization]`, with defaults tuned on the fn-67 bench's AMI dev set. The old `min_coverage` / `min_speaker_share` keys are replaced or reinterpreted, with the migration documented.
- Re-running the speaker pass on an existing meeting (`dettivo meetings diarize <id>`) applies the new rule.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** On the fn-67 bench (AMI dev and the local English and German meetings), the headline word-weighted attribution error falls below today's rule for both the current engine and Nemotron. The pooled unlabelled share of remote lines falls from 22.7% to under 3%.
- **R2:** The wrong-speaker share of labelled lines does not rise above today's rule's by more than 1 point on AMI dev for either engine. The bench reports it.
- **R3:** The AMI test (`--heldout`) figures are recorded in the report and confirm R1 without retuning.
- **R4:** Unit rules have Rust unit tests: sentence ends, pauses, speaker changes across a pause, the nearest-turn tolerance, fragment merging, room-audio meetings and a microphone-only meeting.
- **R5:** The config keys are documented in `docs/config.md` and `docs/meetings.md`, including the migration from the old keys. An ADR amends ADR 0035's labelling rule.
- **R6:** Re-running diarization on an existing meeting relabels it under the new rule without data loss, and a test covers it.

## Boundaries

- No voice check (fn-70) and no engine change (fn-69) here.
- No LLM correction.
- The microphone track stays "You" (two-track rules are fn-71).

## Decision Context

Gordon approved recommendation 1 of the 2026-09-26 research on 2026-09-26. It is the single largest expected gain, and it needs no new model.
