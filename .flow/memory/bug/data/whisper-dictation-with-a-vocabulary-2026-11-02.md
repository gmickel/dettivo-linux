---
title: Whisper dictation with a vocabulary capitalises a near-term common word — 'sapie
date: "2026-11-02"
track: bug
category: data
module: dettivo-engine-whisper vocabulary prompt
tags: [qa, fn-88, whisper, vocabulary]
problem_type: data
symptoms: large-v3-turbo with SapienXT listed types 'Every Sapient creature' for 'Every sapient creature'; vocabulary off types lowercase
root_cause: "(observed via live QA — unconfirmed; likely Whisper's initial-prompt bias, pre-existing)"
resolution_type: fix
related_to: [bug/data/hand-formatted-vocabulary-loses-its-per-2026-11-01]
---

## Problem
A user dictating with Whisper large-v3-turbo and a vocabulary that lists `SapienXT` gets the ordinary word "sapient" capitalised mid-sentence ("Every Sapient creature needs water and rest."). No listed term is inserted; only the case of a near-term common word changes.

## Steps to reproduce (cold)
1. Isolated daemon (goal head 9f9940a, Vulkan release build), `[speech] model = "large-v3-turbo"`, Silero VAD present, mock mic `crates/dettivo-qa/fixtures/vocabulary/distractor-sapient.wav`.
2. `[dictation] vocabulary` = Dettivo, Clef (clay, cleff), SapienXT (sapient x t, sapien xt), Omarchy, Hyprland.
3. `dettivo dictation press`, hold for the clip length, `dettivo dictation release`.
4. Read the text the stand-in compositor's virtual keyboard received.

## Expected
fn-89 R1: "zero false insertions on the distractor clips". The fixture says "Every sapient creature needs water and rest."

## Actual
With the vocabulary: "Every Sapient creature needs water and rest." Without it: "Every sapient creature needs water and rest." The term is not inserted (no false insertion by fn-89's scoring), but the prompt bias changes the case of a non-term word. The same output is already recorded in docs/reports/benchmarks/vocabulary-2026-11-02-thor-sound-check-rendering-2.json, so it predates this goal's later commits.

## Evidence
- .flow/tmp/qa-fn-88-better-dictation-shipped-as-one-release/X6f-distractor-sapient-vocab-vk.log
- .flow/tmp/qa-fn-88-better-dictation-shipped-as-one-release/X6h-distractor-sapient-off-vk.log

## Traceability
- R-IDs: fn-89 R1 (via fn-88 R1)   scenario: X6f/X6h   driver_rung: CLI + stand-in compositor
