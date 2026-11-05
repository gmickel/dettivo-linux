---
title: Whisper vocabulary check panicked on overlapping gaps of adjacent multi-word ter
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-engine-whisper/src/check.rs
tags: [fn-89-whisper-vocabulary-without-false, whisper, vocabulary]
problem_type: runtime-error
symptoms: replace_range panics when two suspect terms' fallback gaps overlap
root_cause: "a word inside a multi-word term anchored both decodes, so per-suspect gaps overlapped and only identical gaps were deduplicated"
resolution_type: fix
related_to: [bug/runtime-errors/parakeet-dictation-user-gets-fewer-2026-11-01, bug/runtime-errors/parakeet-user-at-strong-loses-the-2026-11-02, bug/runtime-errors/vocabulary-rescoring-rewrote-part-of-a-2026-11-01]
---

## Problem
Whisper's vocabulary check (ADR 0095) rewrites a rejected term with what the prompt-free decode heard between the nearest words both decodes share. When a word inside a multi-word term was itself shared ("XT" in "Sapien XT", "New" in "New Hampshire"), two adjacent suspects produced gaps that overlapped. Their byte edits overlapped too, and `String::replace_range` panicked on the second one, which would kill the engine and lose the take. The 47-clip benchmark had no multi-word term next to another term, so it never hit the case; all three reviewers found it by tracing the anchors.

## What Didn't Work
Deduplicating identical gaps (`!gaps.contains(...)`) only covered two suspects that shared both anchors. Applying edits in descending start order is safe only for disjoint ranges.

## Solution
Sort the gaps by our word range and merge any that overlap into their union on both sides before building byte edits (`crates/dettivo-engine-whisper/src/check.rs`, `settle`). Anchors come from a longest common subsequence, so both decodes stay in order and the union is one contiguous gap on each side. Regression: `suspects_with_overlapping_gaps_settle_as_one` in `check_tests.rs`.

## Prevention
Any code that turns several word spans into byte edits on one string must merge or reject overlapping ranges before applying them, and needs a test with adjacent multi-word spans that share a word with the reference text.
