# Whisper's vocabulary term check, 2026-11-02 on thor

This run chooses the confidence a listed term needs to stay in Whisper's output (ADR 0095). Every level from -0.45 to -0.2 nats of mean token log probability keeps Whisper large-v3-turbo's 10 of 15 terms and writes no term where none was said. The check ships at -0.3, the round level nearest the middle of that window. On this set, the vocabulary now gives Whisper 8 more terms than it has without one, for no false insertion, and the guard sentences come out exactly as said.

## The baseline

Whisper large-v3-turbo (`ggml-large-v3-turbo.bin`), release build, Vulkan on an RTX 4090, through `dettivo-qa bench vocabulary` on the 47-clip set before this change, at `e0071ee` ([vocabulary-2026-11-02-thor-whisper-baseline.json](vocabulary-2026-11-02-thor-whisper-baseline.json)). Every clip is at least 1.5 s long, so dictation's padding leaves Whisper's clips as they are.

| Vocabulary | Term recall | False insertions | Guard WER |
|---|---|---|---|
| off | 2/15 | 0 | 0.011 |
| on | 10/15 | 4 | 0.000 |

The four insertions come from three distractors: "Omar keeps the spare keys in his desk." came back as "Omarchy, Hyprland." (two), "Clay and I went to the cinema" as "Clef and I went", and "Omar, he said, would arrive" as "Omarchy said would arrive". The session's whole-prompt echo guard keeps all three, because the echo holds two of the five terms.

## Where each term stands

One debug pass of the check over every clip logs each listed term's mean token log probability and whether it is echo-shaped (`RUST_LOG=debug`, prompt and terms as dictation sends them). The value depends on the prompted decode alone, so it holds at every threshold.

| Clip | Term written | Echo-shaped | Mean log probability | What the decode without the prompt heard |
|---|---|---|---|---|
| target-clef-3 | Clef | no | -0.009 | Cluff |
| target-clef-2 | Clef | no | -0.014 | Clef |
| target-omarchy-2 | Omarchy | no | -0.017 | O Marky |
| target-clef-1 | Clef | no | -0.021 | Clef |
| target-omarchy-1 | Omarchy | no | -0.022 | Omarki |
| target-sapienxt-2 | SapienXT | no | -0.037 | Sapien XT |
| target-sapienxt-1 | SapienXT | no | -0.060 | Sapien XT |
| target-omarchy-3 | Omarchy | no | -0.061 | OMAR he |
| target-sapienxt-3 | SapienXT | no | -0.064 | Sapien XT |
| target-hyprland-1 | Hyprland | no | -0.177 | Piper Lan |
| distractor-clay-name | Clef | no | -0.458 | Play |
| distractor-omar-he | Omarchy | no | -0.660 | Omar, he |
| distractor-omar | Omarchy | yes | -0.961 | Omar keeps the spare keys in his desk. |
| distractor-omar | Hyprland | yes | -1.184 | (nothing) |

The two echo-shaped terms are suspects at any threshold. A target whose decode without the prompt spells the same letters stays at any threshold: the Clef clips 1 and 2, and the three SapienXT clips, where "Sapien XT" spells SapienXT. So the threshold decides five targets and two distractors. The least sure target, target-hyprland-1, sits 0.28 nats above the surest distractor, distractor-clay-name.

## Every level measured

One release Vulkan engine build per level, each through the whole bench with the dictation guards (ADR 0096), filed as `vocabulary-2026-11-02-thor-whisper-threshold<level>.json`. The runs were made from the working tree on `e0071ee`, before the change was committed, so their `git_sha` names the base. The off row was the same in every run: 2/15, no insertion, guard WER 0.011. Other agents shared the GPU (1 to 48 % busy at the start of a run).

| Level | Term recall | False insertions | Guard WER | Clips that needed the second decode, ms added |
|---|---|---|---|---|
| -1.0 | 10/15 | 2 | 0.000 | distractor-omar 78.7 |
| -0.5 | 10/15 | 1 | 0.000 | distractor-omar 82.8, distractor-omar-he 81.0 |
| -0.45 | 10/15 | 0 | 0.000 | the three distractors, 77.6 to 77.8 |
| -0.4 | 10/15 | 0 | 0.000 | the three distractors, 77.1 to 78.0 |
| -0.35 | 10/15 | 0 | 0.000 | the three distractors, 78.1 to 78.4 |
| **-0.3** | **10/15** | **0** | **0.000** | the three distractors, 79.4 to 85.5 |
| -0.25 | 10/15 | 0 | 0.000 | the three distractors, 76.9 to 77.5 |
| -0.2 | 10/15 | 0 | 0.000 | the three distractors, 76.7 to 78.4 |
| -0.15 | 9/15 | 0 | 0.000 | target-hyprland-1 77.7 and the three distractors |
| -0.1 | 9/15 | 0 | 0.000 | target-hyprland-1 77.6 and the three distractors |

At -0.15 and above, target-hyprland-1 is checked and gives way to "Piper Lan". At -0.5, "Clef and I went to the cinema" stays, and at -1.0 "Omarchy said would arrive" stays with it. In between, the three distractors come back as the decode without the prompt heard them: "Omar keeps the spare keys in his desk.", "Play and I went to the cinema on Saturday." and "Omar, he said would arrive at noon."

## The shipped level

-0.3 clears target-hyprland-1 by 0.12 nats and stays 0.16 nats above distractor-clay-name. A run of the committed engine at `60efadb` reproduced the -0.3 row transcript for transcript ([vocabulary-2026-11-02-thor](vocabulary-2026-11-02-thor.md)).

- The 33 clips whose prompted output holds no listed term came back byte for byte as before the change, and none of them was decoded twice.
- 10 of the 13 clips with a term needed no second decode, so the median time added over clips with a candidate is 0.0 ms. The three that did took 78.0 to 84.5 ms more, against the 150 ms target.

The set comes from two synthetic voices. A real speaker's terms will spread wider than these, so the 0.12 nats of room below the last Hyprland clip is the number to watch. The levels that keep every target and keep out every distractor span 0.28 nats.
