---
title: Whisper dictation user can't paste a lone vocabulary name — 'Omarchy.' pastes 'O
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-engine-whisper/src/check.rs (echo shape)
tags: [qa, fn-89-whisper-vocabulary-without-false, whisper, vocabulary]
problem_type: runtime-error
symptoms: "A take that is one listed name pastes the prompt-free misspelling: 1/10 names pasted at HEAD vs 7/10 at base (five-term vocabulary)"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/runtime-errors/duck-ramp-overwrites-a-volume-the-user-2026-11-01, bug/runtime-errors/parakeet-dictation-user-gets-fewer-2026-11-01, bug/runtime-errors/parakeet-user-at-strong-loses-the-2026-11-02, bug/runtime-errors/vocabulary-rescoring-rewrote-part-of-a-2026-11-01, bug/runtime-errors/whisper-vocabulary-check-panicked-on-2026-11-02]
---

## Problem
A Whisper dictation user who says one listed vocabulary name on its own ("Omarchy.", "Clef.", "Hyprland.") no longer gets the name pasted. fn-89's echo-shape rule treats a take that is only listed terms as a prompt echo and replaces it with the prompt-free decode unless that decode also spells the term, so the user gets the misspelling the vocabulary exists to fix ("Oh Marky.", "Pleph.", "Hyperland."), even when the prompted decode was near-certain (Clef at mean log probability -0.009, Omarchy at -0.097). Before fn-89 the same takes pasted the name.

## Steps to reproduce (cold)
1. Render single-name clips with `scripts/qa/make-vocabulary-fixture.py ~/.cache/dettivo/diarize-models <dir>` from a set.json whose clips say "Dettivo.", "Clef.", "Sapien X T.", "Oh marky.", "Hyper land." in the amy and ryan Piper voices (the set used: .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/single/set.json).
2. Isolated profile inside scripts/qa/xvfb-session.sh, debug `dettivod` at 9b1f92e, `DETTIVO_QA_MODE=1 DETTIVO_MOCK_MIC=<clip> DETTIVO_MOCK_INSERT=1`, `[engines] directory` = release Vulkan `dettivo-engine-whisper` at 9b1f92e, `[speech] provider = "whisper"`, `model = "large-v3-turbo"`, `[dictation] language = "en"`, the fixture set's five-term vocabulary (and, separately, a one-term vocabulary of the clip's term).
3. `dettivo dictation start`, wait the clip length + 0.4 s, `dettivo dictation stop`, read `$XDG_STATE_HOME/dettivo/qa/inserted.txt`.
4. Repeat with `dettivod` and the engine built at the base e0071ee.

## Expected
fn-79 R6: "A take that says the one listed term pastes it." Spec fn-89: "listed terms come out when they were said, and never when they were not." docs/dictation.md (Dictation section): "Dictating the one listed term still pastes it."

## Actual
Five-term vocabulary, 10 single-name takes (two voices x five terms):

| Clip | Base e0071ee pasted | HEAD 9b1f92e pasted (run1 / run2) | engine check line (HEAD) |
|---|---|---|---|
| single-clef-amy | Clef. | Pleph. / Pleph. | candidates=1 suspects=1 replaced=1, echo=true mean_logprob=-0.009 |
| single-clef-ryan | Clef. | Blah. / Blah. | candidates=1 suspects=1 replaced=1 |
| single-omarchy-amy | Omarchy. | Oh Marky. / Oh Marky. | echo=true mean_logprob=-0.097, replaced=1 |
| single-omarchy-ryan | Omarchy. | Oh Marky. / Oh Marky. | replaced=1 |
| single-hyprland-amy | Hyprland. | Hyperland. / Hyperland. | echo=true mean_logprob=-0.411, replaced=1 |
| single-hyprland-ryan | Hyperland. | Hyperland. / Hyperland. | candidates=0 |
| single-sapienxt-amy | SapienXT | SapienXT / SapienXT | replaced=0 (prompt-free "Sapien XT" spells it) |
| single-sapienxt-ryan | SapienXT. | Sapient XT / Sapient XT | replaced=1 |
| single-dettivo-amy | Edevo. | Edevo. / Edevo. | candidates=0 |
| single-dettivo-ryan | Ettevo. | Ettevo. / Ettevo. | candidates=0 |

Names pasted: base 7/10, HEAD 1/10. Every one of the six names Whisper heard with the prompt but not without it was removed. With a one-term vocabulary: base 1/10 (Hyprland, amy), HEAD 0/10 (that take now "Hyperland."). The vocabulary-off row writes the same misspellings as HEAD, so for a lone name the vocabulary now does nothing on Whisper. No crash, nothing lost beyond the term spelling.

docs/dictation.md line 48 still promises "Dictating the one listed term still pastes it."; line 82 and ADR 0095 Consequences describe the new behaviour.

## Evidence
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/single-on/results.tsv, run2/single-on/results.tsv (HEAD, five terms, two runs)
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/base/single-on/results.tsv (base e0071ee, five terms)
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/single-one-*/results.tsv, base/single-one-*/results.tsv (one-term vocabulary)
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1/single-off/results.tsv (vocabulary off)
- per take: <dir>/<clip>/engine.log (vocabulary candidate echo/mean_logprob), daemon.log, history-latest.json, inserted.txt
- .flow/tmp/qa-fn-89-whisper-vocabulary-without-false/run1-all.log, run2-all.log

## Traceability
- R-IDs: none (regression of fn-79 R6 and of the spec goal "listed terms come out when they were said")   scenario: S6-single-name   driver_rung: CLI + daemon socket (mock mic, mock insert)   viewport: n/a
