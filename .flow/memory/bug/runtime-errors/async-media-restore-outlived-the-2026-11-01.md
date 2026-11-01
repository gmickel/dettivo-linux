---
title: Async media restore outlived the dictation slot; a meeting could start under it
date: "2026-11-01"
track: bug
category: runtime-errors
module: crates/dettivod/src/media.rs
tags: [media, meetings, concurrency, fn-85]
problem_type: runtime-error
symptoms: media ramp or pause still running when a meeting starts after dictation stop
root_cause: Media::ended only queued the restore; finish_shared freed the slot before it ran
resolution_type: fix
---

## Problem
fn-85 moved `[audio] while_dictating` (pause, duck, mute) onto a media thread so capture never waits for it. The review found the end of a take only queued the restore. The session freed its slot at once, so `meetings.start` could begin recording the default sink's monitor during the 200 ms restore ramp. That breaks the spec's rule that nothing changes media while a meeting captures.

## What Didn't Work
Waiting for an acknowledgement with a 2 s cap. A slow MPRIS `PlaybackStatus` call (3 s timeout) let the cap expire, so the slot still freed while a pause was pending.

## Solution
`Media::ended` sends an acknowledgement channel and waits for it with no cap (`crates/dettivod/src/media.rs`). It is called from the publisher inside `finish_shared` before the slot clears, so the slot stays taken until media is back. Each MPRIS call keeps its own timeout, and each `wpctl` run is killed after 2 s, so the wait stays bounded.

## Prevention
When work moves off a path onto a thread, check every guarantee the old synchronous order gave (here: session exclusion covered the restore). A bounded wait that releases on timeout gives no exclusion; bound the work itself instead.
