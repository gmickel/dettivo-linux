---
title: A dictation stop boundary cannot be marked from the worker; end the capture inst
date: "2026-11-01"
track: bug
category: runtime-errors
module: crates/dettivo-session/src/worker.rs
tags: [dictation, capture, stop, race]
problem_type: runtime-error
symptoms: post-release audio reaches the speech gate
root_cause: stop only queues Cmd::Stop; the mock checked stop before its sleep
resolution_type: fix
---

## Problem
fn-79's R1 asked the level gate to leave out audio recorded after the dictation release. The review kept finding post-release audio in the gated take: a stop request only queues `Cmd::Stop` to the PipeWire thread, and the mock microphone checked its stop flag before its real-time sleep, so a stop during the sleep still published the next chunk.

## What Didn't Work
Marking the release inside the take from the session worker. The worker cannot tell a queued chunk recorded before the release from one recorded after it, so each attempt moved the error to the other side of the mark. Four review rounds stalled on it.

## Solution
End the capture from the thread that takes the stop request (`Source::stopper`, `Session::stop_expected`) and gate the whole take. `MockCapture` checks the stop after its wait (`crates/dettivo-audio/src/mock.rs`). The residual PipeWire race, at most one capture quantum before `Cmd::Stop` runs, is an accepted tolerance recorded in ADR 0082 by Gordon.

## Prevention
For a sample-exact boundary across threads, ask early whether a bounded race is acceptable and record it in an ADR, instead of looping fixes against an absolute wording. Test stop boundaries through the real source's stop path (MockCapture), not only a synchronous channel source.
