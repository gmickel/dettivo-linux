---
title: "Silent-ending window anchored to the rounded cut, not the release"
date: "2026-11-06"
track: bug
category: runtime-errors
module: crates/dettivo-session/src/worker/ending.rs
tags: [release-tail, silero, window, boundary]
problem_type: runtime-error
symptoms: A word ending 136 ms before the release lost its 200 ms tail
root_cause: The 150 ms stretch ended at the 512-sample window boundary after the release instead of at the release
resolution_type: fix
---

## Problem
fn-93's early end of the release tail judged "the last 150 ms" ending at the cut, the first 512-sample Silero window boundary at or after the release. That boundary is up to 32 ms past the release, so the judged stretch began up to 32 ms late: a word ending 136 ms before the release fell outside it and the take dropped its 200 ms tail. The gpt-6.1-sol review caught it in round 1.

## What Didn't Work
Anchoring both tests (the level window and `voiced_until + 150 ms <= end`) to `end`, the sample where the take is cut. The R2 test asserted the full tail only for words ending 60 and 100 ms before the release, so the 140 ms case passed while dropping the tail.

## Solution
`crates/dettivo-session/src/worker/ending.rs` keeps the release sample separately from the cut: the level window runs from `release - 150 ms` to `end`, and a voice answer counts as silent only when `voiced_until + 150 ms <= release`. `crates/dettivo-session/tests/silent_ending.rs` asserts every sample and the whole tail for words ending 60, 100 and 140 ms before the release.

## Prevention
When a rule is stated relative to an event (the release), keep that event's position as its own value and never derive the rule's window from a rounded or delayed position used for cutting or feeding. Boundary tests for such a window assert the decision itself at a point just inside the bound, not only the audio kept.
