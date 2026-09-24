# Cleanup: speech and text quality (7 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

The engines and the language layer never remove or alter speech the user said: silence gating, seam merging, protected tokens and numbers hold under test. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0048 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### engines

- **engines/F4** (bug): Whole-chunk RMS can classify audible speech as silence
  - Where: `crates/dettivo-transcribe/src/job.rs:219` skips recognition when `filters::is_near_silent(&pcm, ...)` succeeds. `crates/dettivo-transcribe/src/filters.rs:96` implements that as `rms(samples) < floor`.
  - Why: The default chunk is five minutes. One second of speech at RMS 0.1 followed by 299 seconds of silence averages to approximately 0.00577, below the 0.0065 threshold. The job skips the speech and can report `notice = silent`. Applying a speech activation threshold to a five-minute average is the wrong assumption.
  - Change: Reject a chunk only when every short analysis window falls below the speech threshold. Retain the cheap shortcut for genuinely silent audio.
  - Risk: More sparse recordings will reach inference. Test a brief utterance surrounded by several minutes of silence alongside the all-silence fixture.

- **engines/F5** (bug): Seam deduplication deletes genuine repetitions across gaps
  - Where: `crates/dettivo-transcribe/src/merger.rs:119` matches `tail[tail.len() - k..] == head[..k]` without checking time. Lines 181–183 invoke reconciliation even when consecutive chunks do not overlap.
  - Why: Matching words alone do not establish duplicate audio. Two separate “yes” utterances can collapse into one even when their windows are seconds apart. The live path retains earlier windows across silence, so this affects ordinary meetings.
  - Change: Bypass reconciliation for disjoint windows. Restrict seam candidates to the actual shared audio interval, with an explicit timestamp tolerance.
  - Risk: Some previously hidden engine repetitions may reappear. Test repeated phrases across silence, zero-overlap chunks and genuine duplicated overlap.

- **engines/F12** (bug): Streamed LLM text disagrees with the final answer when a stop string spans tokens
  - Where: `crates/dettivo-engine-llm/src/engine.rs:315` checks `stop_at(&text, ...)`, then line 320 emits `partial(&piece)` for preceding pieces.
  - Why: A prefix of a stop string can already have been emitted when its remaining characters arrive. The release probe with stop string `"3\n"` streamed `"3"` but returned an empty final answer.
  - Change: Hold back the longest suffix that could begin a stop string. Emit it only when it can no longer match; discard it when the stop completes.
  - Risk: Streaming gains a small bounded delay. Test stop strings spanning multiple tokens, Unicode boundaries and ordinary end-of-generation flushing.

### dictation

- **dictation/F12** (bug): Polish edits inside tokens that Raw deliberately protected
  - Where: `crates/dettivo-language/src/polish/mod.rs:35` — `collapse_whitespace(trimmed)`; `crates/dettivo-language/src/polish/mod.rs:68` — global filler replacement; `crates/dettivo-language/src/polish/mod.rs:100` — replacing `r"\bi\b"` with `"I"`; `crates/dettivo-language/src/polish/mod.rs:121` — `text.to_lowercase()`.
  - Why: Raw restores protected spans before handing text to Polish. Polish then applies unrestricted transformations to the whole string. A backticked command containing `um` loses it, a path component `i` can become `I`, and Very Casual lowercases case-sensitive tokens.
  - Change: Share protected-span handling across all deterministic stages. Apply prose transformations only outside protected spans and preserve structural whitespace where it matters.
  - Risk: Existing goldens may encode these mutations. Add full-pipeline cases for backticked commands, case-sensitive paths and identifiers under every style.

- **dictation/F17** (bug): The numeric preservation guard accepts changed numbers
  - Where: `crates/dettivo-language/src/enhanced/guard.rs:106` — `output.chars().filter(char::is_ascii_digit).collect()`; `crates/dettivo-language/src/enhanced/guard.rs:127` — `if !output_digits.contains(&digits)`.
  - Why: Concatenating every output digit discards number boundaries and punctuation. Input `42` is considered preserved in `420`; `12` can be satisfied by separate output numbers `1` and `2`; `1.5` can be satisfied by `15`. Repeated input numbers also reuse one output occurrence.
  - Change: Compare numeric tokens and their multiplicity, preserving signs and decimal meaning. Allow only explicitly supported formatting equivalences.
  - Risk: Locale-specific grouping requires a defined policy. Test changed magnitude, decimals, signs, repeated numbers and separate numbers whose concatenated digits match.

- **dictation/F18** (bug): Silence detection depends on the UI meter interval
  - Where: `crates/dettivo-session/src/worker.rs:109` — `peak < self.policy.silence_peak_threshold`; `crates/dettivo-session/src/worker.rs:299` — only `Event::Level` updates peak; `crates/dettivo-audio/src/level.rs:32` — emission requires `self.count >= self.window`.
  - Why: The worker derives silence from completed meter windows rather than captured PCM. A voiced take shorter than the configured meter interval can contain samples but no level event, leaving peak at zero and skipping transcription. Speech confined to the final partial window has the same problem.
  - Change: Calculate the take’s silence statistic directly from PCM as it is collected, independently of display-level events.
  - Risk: Keep amplitude normalization consistent with the existing threshold. Test short speech, silence followed by a voiced partial window, and different meter intervals over identical PCM.

### qt-hosts

- **qt-hosts/F2** (bug): Meeting notes stop being protected before the daemon confirms their save
  - Where: `qt/host/app/meeting_detail_model.cpp:415` and `qt/host/app/meeting_live_model.cpp:257`, both `m_notesDirty = false;`; `qt/host/app/meeting_detail_model.cpp:140`, `if (!m_notesDirty)`.
  - Why: Both models clear the dirty flag before sending the save. A failure leaves it cleared, so neither retries that edit. The detail model also accepts incoming notes whenever that flag is clear, allowing a reload during an outstanding save to overwrite the local draft. Closing the application before the debounce fires has no notes-flush path.
  - Change: Track the draft revision separately from the acknowledged revision. Preserve failed and outstanding drafts, serialize saves per meeting, and finish or retain pending edits before switching meetings or closing. Share this small save mechanism between the two models.
  - Risk: Late acknowledgements must not mark newer text saved or affect another meeting. Test delayed replies, disconnects, rapid edits, meeting switches and immediate window closure.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** engines/F4, Whole-chunk RMS can classify audible speech as silence: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** engines/F5, Seam deduplication deletes genuine repetitions across gaps: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** engines/F12, Streamed LLM text disagrees with the final answer when a stop string spans tokens: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** dictation/F12, Polish edits inside tokens that Raw deliberately protected: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** dictation/F17, The numeric preservation guard accepts changed numbers: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** dictation/F18, Silence detection depends on the UI meter interval: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** qt-hosts/F2, Meeting notes stop being protected before the daemon confirms their save: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0048 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
