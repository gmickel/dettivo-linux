# Whisper's lone-name sound check, 2026-11-02 on thor

This run sets how alike a lone name and the prompt-free decode must sound for the name to stay (ADR 0095). The check ships at 0.6. Every level from 0.55 to 1.0 keeps the most names reachable without a false insertion: 6 of 10 on the committed single-name set, and 5 and 6 of 10 on two fresh renderings, with none of the 10 sound-alikes said alone becoming a term. At 0.533 or below, the lone "Clay." (amy) on the committed set pastes "Clef". No level reaches 7 of 10 on the committed set without that false insertion.

Whisper large-v3-turbo over Vulkan on an RTX 4090, `dettivo-qa bench vocabulary` at `fea561b`. The GPU was shared with other users at 9 to 95 % load, so the times are indicative. Each rendering is the single-name set of `crates/dettivo-qa/fixtures/vocabulary/single/` rendered by `scripts/qa/make-vocabulary-fixture.py` with the same Piper voices. Renderings 2 and 3 are fresh syntheses (new VITS noise) that were swapped into the set for one run each and not committed. The 47-clip set holds no lone name the check doubts, so its transcripts are byte for byte those of [vocabulary-2026-11-02-thor-single-name](vocabulary-2026-11-02-thor-single-name.md) at every level: 10 of 15 terms, no false insertion in 25 distractors, guard WER 0.000 against 0.011 with the vocabulary off.

## The lone names the sound check decided

A lone name reaches the sound check only when it is unsure (mean token log probability under -0.3) and the prompt-free decode heard other letters. The score is the best `sound::alike` over the term and its `sounds_like` spellings.

| Rendering | Clip | Prompted | Log prob | Prompt-free decode | Best score | Kept at 0.6 |
|---|---|---|---|---|---|---|
| 1 (committed) | single-hyprland-amy | Hyprland. | -0.411 | Hyperland. | 1.000 (alias `hyperland`) | yes, a name |
| 1 (committed) | single-clef-ryan | Clef. | -0.416 | Blah. | 0.375 | no |
| 1 (committed) | lone-clay-amy | Clef, | -0.344 | Clyde. | 0.533 (`clef`, `cleff`) | no, a sound-alike |
| 2 | single-hyprland-amy | Hyprland. | -0.386 | Hyperland | 1.000 (alias `hyperland`) | yes, a name |
| 3 | single-omarchy-ryan | Omarchy. | -0.496 | Oh Marky. | 1.000 (alias `oh marky`) | yes, a name |
| 3 | single-clef-ryan | Clef. | -0.524 | Bye. | 0.000 | no |

SapienXT (amy, rendering 1, -0.415) was unsure too, but the prompt-free decode spelt it as well, so it stood without a score. Every other sound-alike in the three renderings came back without a term from the prompted decode itself.

## Levels

Names pasted out of 10 and sound-alikes that became a term out of 10, per rendering. "Off" is the check before this change, which never keeps an unsure lone name.

| Level | Rendering 1 | Rendering 2 | Rendering 3 | Safe on all three |
|---|---|---|---|---|
| off | 5, 0 | 4, 0 | 5, 0 | yes |
| 0.0 | 7, 1 | 5, 0 | 7, 0 | no |
| 0.3 | 7, 1 | 5, 0 | 6, 0 | no |
| 0.4 | 6, 1 | 5, 0 | 6, 0 | no |
| 0.5 | 6, 1 | 5, 0 | 6, 0 | no |
| 0.533 | 6, 1 | 5, 0 | 6, 0 | no |
| 0.55 | 6, 0 | 5, 0 | 6, 0 | yes |
| **0.6** | **6, 0** | **5, 0** | **6, 0** | **yes** |
| 0.65 | 6, 0 | 5, 0 | 6, 0 | yes |
| 0.7 | 6, 0 | 5, 0 | 6, 0 | yes |
| 0.8 | 6, 0 | 5, 0 | 6, 0 | yes |
| 0.9 | 6, 0 | 5, 0 | 6, 0 | yes |
| 1.0 | 6, 0 | 5, 0 | 6, 0 | yes |

The safe window runs from just above 0.533, the surest sound-alike, to 1.0, the score of every name the check kept. The level is 0.6, the midpoint between that sound-alike and the 0.667 that "Pleph" scores against Clef, the swapped-sound case the decision named as one to keep. It has 0.067 above the surest sound-alike and 0.4 below every name kept on these renderings. The names it cannot reach were heard as something else entirely: Clef (ryan) as "Blah." and "Bye.", and Dettivo, which the prompted decode never wrote.

The levels for renderings 1 to 3 were read from each lone suspect's debug-level `vocabulary lone name sound=<score>` line. The shipped level was confirmed end to end by the bench on all three ([main](vocabulary-2026-11-02-thor-sound-check.md), [rendering 2](vocabulary-2026-11-02-thor-sound-check-rendering-2.json), [rendering 3](vocabulary-2026-11-02-thor-sound-check-rendering-3.json)). The sound check adds no decode: it scores words the confidence check had already decoded. The second decodes on the single-name set took 69 to 81 ms over the three renderings.
