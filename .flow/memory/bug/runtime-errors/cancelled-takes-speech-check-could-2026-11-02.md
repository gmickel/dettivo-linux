---
title: Cancelled take's speech check could reset the next take's VAD stream
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-session/src/worker/check.rs
tags: [speech-gate, vad, concurrency, cancel]
problem_type: runtime-error
symptoms: A cancel then a new take could give the new take another take's speech verdict
root_cause: The check thread was detached on drop and its running piece reached the shared VAD process after the next take started
resolution_type: fix
related_to: [bug/runtime-errors/async-media-restore-outlived-the-2026-11-01]
---

## Problem
fn-90 moved the dictation speech check onto a thread that feeds the Silero VAD process while the take is captured. That process holds one take's stream. When a take ended without a verdict (a cancel), the worker dropped the check and detached its thread. A piece the thread was still running could land after the next take started: its `feed_speech` with `start: true` reset the new take's stream. When the two takes had fed the same sample count, the count checks passed and the next take got the other take's verdict. The gpt-6.1-sol review replayed that ordering (0 ms of speech against 7,600 ms).

## What Didn't Work
Setting a `finishing` flag in `Drop` and letting the thread stop on its own. The flag only takes effect at the next piece, not the one already running, and nothing ordered that piece before the session became available again.

## Solution
`SpeechCheck::drop` (crates/dettivo-session/src/worker/check.rs) closes the channel and joins the thread. Every take's check has therefore finished before `finish_shared` lets another take start. The capture loop never reaches that drop, so capture still never waits. Regression: `a_cancelled_take_check_ends_before_the_next_take_starts` in crates/dettivo-session/tests/speech_check.rs.

## Prevention
A background thread that talks to a process holding per-session state must end before the session does: join it on every exit path, not only the success path. A count or length check is no take identity, because two takes can have equal counts. Test the cancel-then-new-take ordering with a deliberately slow piece.
