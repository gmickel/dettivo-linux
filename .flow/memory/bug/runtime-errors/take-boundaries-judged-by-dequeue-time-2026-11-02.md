---
title: "Take boundaries judged by dequeue time let a queued release keep audio past its "
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-session/src/worker/record.rs
tags: [dictation, capture, timestamps, ring]
problem_type: runtime-error
symptoms: a release queued behind a slow start keeps audio after release + 200 ms
root_cause: "chunks stamped by the consumer on dequeue, written as they arrived"
resolution_type: fix
related_to: [bug/runtime-errors/a-dictation-stop-boundary-cannot-be-2026-11-01]
---

## Problem
fn-81's first review found that a dictation take's boundaries were judged by when the session worker dequeued each PCM chunk. A release that queued behind a slow start (a cold engine, `pause_media`) let every chunk the always-ready ring delivered after the release into the take file and the engine, because the worker stamped the backlog with its own receive time and wrote chunks as they arrived.

## What Didn't Work
Stamping chunks on receipt in the consumer. That is the same trap as the fn-79 stop-boundary entry: a consumer cannot tell a chunk that waited in its channel from one captured late. The release tail looked right in tests that fed chunks in real time and broke only when the worker lagged.

## Solution
`Event::Pcm { pcm, at }` (`crates/dettivo-audio/src/lib.rs`) carries the capture time: the PipeWire process callback and the mock stamp it, and the ready feed forwards it unchanged. The worker keeps `(end index, at)` per chunk and writes the take once, after `cut_at` (`crates/dettivo-session/src/worker/record.rs`) trims it at `released_at + release_tail` to the sample.

The same review found three ownership gaps in the ring manager (`crates/dettivod/src/ready.rs`):
- The open feed never compared its device with `[audio] input_device`.
- "A dictation is running" closed the ring during transcription.
- A refused start left its tap attached.

## Prevention
Put timestamps where audio is produced, never where it is consumed. Pin a boundary with a test whose consumer starts late (`a_release_queued_behind_a_slow_start_ends_the_take_at_its_tail`). For any cached resource such as the ring, list each input that should invalidate it (configuration, device presence, the session state) and give each one a reconcile test.
