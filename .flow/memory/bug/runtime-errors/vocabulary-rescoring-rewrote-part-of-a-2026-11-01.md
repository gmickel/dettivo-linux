---
title: Vocabulary rescoring rewrote part of a multi-word term already heard right
date: "2026-11-01"
track: bug
category: runtime-errors
module: crates/dettivo-engine-parakeet/src/bias/candidates.rs
tags: [parakeet, vocabulary, rescoring]
problem_type: runtime-error
symptoms: Guard sentence became 'because of the the holiday' with a multi-word term listed
root_cause: Case-only guard skipped only the exact span; overlapping sub-spans stayed candidates
resolution_type: fix
---

## Problem
Parakeet's vocabulary rescoring skipped a span whose words already spelled a multi-word term in another case, but a shorter span inside it stayed a candidate. With `because of the` in the vocabulary, the guard sentence "...because of the holiday" became "because of the the holiday": the word `the` alone matched the term and was rewritten into the whole term.

## What Didn't Work
A per-span `continue` when the span equals the term (case-insensitive). It protects only that exact span, never its sub-spans or overlapping spans.

## Solution
`candidates::find` (crates/dettivo-engine-parakeet/src/bias/candidates.rs) first collects every span that already is a term (any case, or the term or an alias plus `s`/`es`), then drops every candidate for that term that overlaps one. A regression test covers `because of the`, `o Markey`, `Rain Is` and `Sapien XTs`.

## Prevention
For any span-based rewrite, test multi-word targets that already appear correctly in the input, and assert no candidate overlaps them; a guard on the exact span alone is not enough.
