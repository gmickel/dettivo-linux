# Vocabulary boost levels, 2026-11-01 on thor

This run sets Parakeet's `vocabulary_boost` levels. At 4.0 nats per piece, `normal` brings back 8 of the 12 terms Parakeet misses and writes no term where none was said. At 5.0, `strong` brings back 10. At 6.0, the bench gets 11 back but writes Hyprland over "hyper landing" in an ordinary sentence. So `light` is 2.5, `normal` 4.0 and `strong` 5.0 (ADR 0088).

## The set

The fixture set grew from 24 clips to 47, so false insertions are measured on more than five sentences. The 15 target clips are unchanged byte for byte, so recall compares directly with the fn-83 baseline ([vocabulary-2026-11-01-thor](vocabulary-2026-11-01-thor.md)) and the first rescoring run ([vocabulary-2026-11-01-thor-rescoring](vocabulary-2026-11-01-thor-rescoring.md)).

- 24 distractor clips, at least four per term, each a near-miss name or a sound-alike common word:
  - Dettivo: detective (twice), "the TV, oh", "debt every", TiVo.
  - Clef: clay, cleft, clap, Clay as a name, chef.
  - SapienXT: sapient, "Sapiens next", "sapiens extended", "Damien next".
  - Omarchy: "Omar keeps", "Omar, he said", "Oh, Marky", "on March eighth", monarchy, anarchy.
  - Hyprland: highland, Highlands, "hyper landing", hyperlink, heartland.
- 7 guard sentences, three of them 16 to 19 words long.

`scripts/qa/make-vocabulary-fixture.py` rendered the 23 new clips with the same two Piper voices. The script now renders only clips whose `.wav` is missing, because a second synthesis of the same text differs byte for byte. Parakeet with the vocabulary off heard the new clips as written, apart from case and "March 8", except "Play and I" for "Clay and I", "TVO" for "TV, oh", "Tyvo" for TiVo and "Marquis" for Marky. In 17 of the 24 distractors, at least one span became a rescoring candidate, so the boost was tested against them.

## Results

Parakeet v3 (`tdt-0.6b-v3-q8_0`), release build, Vulkan, at `3c9c02e` plus the new fixtures. The bench runs its `on` pass at the `normal` level, so each boost ran from its own engine build with `normal` mapped to that boost. The four runs are filed as `vocabulary-2026-11-01-thor-boost-<boost>.json`. Other agents were using the GPU during the runs.

| Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER | Rescoring added |
|---|---|---|---|---|---|
| off | 3/15 | 0 of 12 | 0 | 0.000 | - |
| 2.5 | 8/15 | 5 of 12 | 0 | 0.000 | 14.5 ms median on 29 clips, 1.0 ms most without |
| 4.0 | 11/15 | 8 of 12 | 0 | 0.000 | 14.4 ms median on 29 clips, 1.4 ms most without |
| 5.0 | 13/15 | 10 of 12 | 0 | 0.000 | 15.4 ms median on 29 clips, 1.2 ms most without |
| 6.0 | 14/15 | 11 of 12 | 1 | 0.000 | 14.2 ms median on 29 clips, 1.1 ms most without |

All four off passes scored the same: 3/15, no insertion, guard WER 0.000.

- 4.0 adds `dativo` to Dettivo, `O Markey` to Omarchy and `Hyperland` to Hyprland, over 2.5.
- 5.0 adds the other two Hyprland clips (`Hyperland` and `HyperLAN`).
- 6.0 adds `Itivo` to Dettivo and turns "The kids were hyper landing" into "The kids were Hyprland".
- `DeDivo` for Dettivo stays a miss at every boost, 3.8 nats short at 5.0.

## How close the distractors came

The engine's debug line gives each candidate's margin, `log P(term) + boost x pieces - log P(heard)`. A positive margin replaces the heard words. At 5.0 the nearest distractor is "hyper landing", 3.2 nats short of Hyprland over 4 pieces. That gap is 0.8 nats per piece, so the 6.0 run crossed it, as measured. The next nearest are "Omar, he" at 11.3 nats short, then anarchy, hyperlink and chef at 15 to 17 nats short. `strong` therefore leaves less room than `normal`: 4.0 keeps "hyper landing" 7.2 nats short.
