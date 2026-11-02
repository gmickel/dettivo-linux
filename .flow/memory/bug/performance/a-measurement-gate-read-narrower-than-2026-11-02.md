---
title: A measurement gate read narrower than its text hid the takes that met it
date: "2026-11-02"
track: bug
category: performance
module: docs/adr/0094 (fn-86 streaming gate)
tags: [measurement-gate, streaming, parakeet, adr]
problem_type: performance
symptoms: "ADR declared fn-86's gate not met while 120 s takes measured 1.08-1.23 s against a 30 s-or-longer, 1 s gate"
root_cause: "The gate's unbounded take length was narrowed to 30-60 s, the lengths that favoured closing the spec"
resolution_type: documentation
---

## Problem
fn-86's gate said "build streaming only if release-to-text for 30 s or longer takes exceeds 1 s". The first ADR 0094 read it as "30 s and 60 s takes" and declared the gate not met. That reading dropped the measured 120 s takes (1,079 and 1,234 ms), which the text covers. The gpt-6.1-sol review flagged it as P1 against R1.

## What Didn't Work
Narrowing an unbounded gate ("30 s or longer") to the lengths that favoured the cheaper outcome, and treating a cheaper alternative fix (moving the serial Silero gate) as if it changed the measured result.

## Solution
The corrected ADR 0094 recorded the gate as met at 120 s and not met at 30 s or 60 s, the three lengths measured, and escalated instead of closing: it stayed Proposed with fn-86 open, and gave a human the build-or-close call with both options and the evidence. That evidence includes parakeet.cpp v0.5.0 streaming only cache-aware models, and the TDT dictation GGUFs carry no parakeet.streaming.* keys.

Gordon Mickel chose option B on 2026-11-02. ADR 0094 (docs/adr/0094-streaming-dictation-meets-its-gate-at-120-s-and-not-at-30-or-60-s.md) is Accepted, fn-86 closed without building streaming, and fn-90 carries the follow-on: running the Silero speech check during the take and fixing the stalls of the first take after idle.

## Prevention
Read a measurement gate by its literal bounds; when the measured result splits across the bound, report the split and escalate instead of choosing a narrower reading. Before accepting a spec's mechanism, check that the pinned engine supports it for the models in use (here: parakeet_capi_stream_begin refuses offline models).
