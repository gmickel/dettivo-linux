---
title: Parakeet user at strong loses the possessive of a vocabulary term — Hyprland's b
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-engine-parakeet (vocabulary rescoring)
tags: [qa, fn-84-vocabulary-on-parakeet, parakeet, vocabulary]
problem_type: runtime-error
symptoms: "At vocabulary_boost strong (6.0), dictating \"Hyper land's config\" inserts \"Hyprland config\"; normal and off keep \"Hyperlans config\""
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/parakeet-dictation-user-gets-fewer-2026-11-01, bug/runtime-errors/vocabulary-rescoring-rewrote-part-of-a-2026-11-01]
---

## Problem
A Parakeet user at `vocabulary_boost = "strong"` who dictates the possessive of a vocabulary term ("Hyprland's config") gets the bare term back ("Hyprland config"): the possessive `s` is dropped. The docs promise that a plural or possessive of a term stays as heard. Parakeet hears the possessive as "Hyperlans" (no apostrophe), which is not the term plus `s`, so the never-candidate rule does not catch it, and at 6.0 nats per piece the term wins the span. At `normal` (5.0) and below the heard word stays. The prior `strong` (5.0) did not do this; the raise to 6.0 introduced it.

## Steps to reproduce (cold)
1. Isolated profile, debug `dettivod` at 1ded7cc inside scripts/qa/xvfb-session.sh, `DETTIVO_QA_MODE=1 DETTIVO_MOCK_MIC=.flow/tmp/qa-fn-84-vocabulary-on-parakeet/s5-synth/r5-possessive.wav DETTIVO_MOCK_INSERT=1`, `[engines] directory` = the release Vulkan engines built at 1ded7cc, `[speech] provider = "parakeet"`, `model = "parakeet-v3"`, `[dictation] language = "en"`, the fixture set's five-term vocabulary (Hyprland with sounds_like "hyper land", "hyperland"), `vocabulary_boost = "strong"`.
2. The clip says "Hyper land's config is short." (Piper amy, scripts/qa/make-vocabulary-fixture.py from s5-synth/set.json).
3. `dettivo dictation start`, wait the clip length, `dettivo dictation stop`, read `inserted.txt`.

## Expected
R5: "Case-only differences, inflections, paths, URLs and addresses are never rewritten". docs/dictation.md: "A case-only difference, a plural or possessive of the term, and anything inside a path, URL or email address stay as heard".

## Actual
strong: "Hyprland config is short." (twice through the daemon). normal and off: "Hyperlans config is short.". Engine CLI on the same clip, Hyprland vocabulary: margin at strong +1.78 nats raw, +1.42 padded (replaced=1); at normal -2.22 raw, -2.58 padded (not replaced). Plural ("Both hyper lands crashed") stays "Hyperlands" at strong; paths, URLs, email, case and short terms are untouched at strong.

## Evidence
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/r2/possessive-daemon.log (daemon: strong, normal, off)
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/r2/run2/s5-daemon-strong/r5-possessive/inserted.txt
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/r2/s45/sweep.log (margins off/light/normal/strong, raw and padded)
- .flow/tmp/qa-fn-84-vocabulary-on-parakeet/r2/s45/r5-possessive-strong-pad.log

## Traceability
- R-IDs: [R5]   scenario: S5   driver_rung: CLI + daemon socket (mock mic)   viewport: n/a
