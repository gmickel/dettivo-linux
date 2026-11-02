# 0089. Decode-time vocabulary biasing is not built, because it weighs the same scores rescoring already weighs

Status: Accepted 2026-11-01, levels amended 2026-11-02

## What this gives you

You know where Parakeet's vocabulary support stands and what would move it. Rescoring after the decode (ADR 0088) brings back 10 of the 12 terms Parakeet misses on the benchmark at the default `normal` level, with no term written where it was not said. A second biasing stage inside the decoder would not raise those numbers, so it is not built.

## Situation

fn-84 staged the work. Stage 1 rescores the greedy transcript (ADR 0088). Stage 2, decode-time biasing, was to be built only if stage 1 missed the recall target at `normal`: at least two thirds of the terms Parakeet misses recovered, with no false insertion and no rise in guard word error rate. Stage 2 carries shadow hypotheses beside the greedy path that spell a term through a token trie wherever its first piece is plausible at a word start, gives each piece the full boost when the greedy words sound like the term and 0.6 x the boost otherwise, and lets the term replace the greedy span at the word boundary only if it scores higher.

The benchmark ([vocabulary-2026-11-01-thor-rescoring](../reports/benchmarks/vocabulary-2026-11-01-thor-rescoring.md), Parakeet v3 on Vulkan at `2e1f0f4`, the GPU shared with other benchmark runs) measured stage 1 at the levels the spec proposed:

| Proposed level | Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER |
|---|---|---|---|---|---|
| off | 0 | 3/15 | 0 of 12 | 0 | 0.000 |
| light | 2.0 | 6/15 | 3 of 12 | 0 | 0.000 |
| normal | 2.5 | 8/15 | 5 of 12 | 0 | 0.000 |
| strong | 4.0 | 11/15 | 8 of 12 | 0 | 0.000 |

At the levels the spec proposed, `normal` (2.5) missed the two-thirds target (8 of 12). Every target clip produced a candidate, so matching is not what fails. The scores are: each missed term's forced score sat 2.3 to 8.8 nats short of the heard words after the `normal` boost (`dativo` for Dettivo 2.3, `O Markey` for Omarchy 3.7, `Hyperland` 5.9 and 7.1, `HyperLAN` 8.0, `DeDivo` 8.8). The five distractors of that set sat 24 to 47 nats short at 4.0.

## Decision

- Stage 2 is not built. A shadow hypothesis is scored by the same prediction and joint networks over the same frames, with the same boost per piece or less. Stage 1 already sums every TDT alignment of the term over a window padded 4 frames on each side, which bounds what any single decode-time path through those frames can score. A hypothesis that starts at the same word boundary cannot beat that sum, and the gaps above are the model's own preference for a known spelling.
- The boost carries the rest. Gordon decided on 2026-11-01 that `normal` must meet the target, and the benchmark was rerun with 25 distractors and 7 guard sentences. The dictation path then showed that the pad a take gets before the engine moves the margins, so the bench now pads each clip the same way, and the levels are set from the padded run ([vocabulary-2026-11-01-thor-padded](../reports/benchmarks/vocabulary-2026-11-01-thor-padded.md)):

| Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER |
|---|---|---|---|---|
| off | 3/15 | 0 of 12 | 0 | 0.000 |
| 2.5 | 8/15 | 5 of 12 | 0 | 0.000 |
| 4.0 (`light`) | 10/15 | 7 of 12 | 0 | 0.000 |
| 5.0 (`normal`) | 13/15 | 10 of 12 | 0 | 0.000 |
| 6.0 (`strong`) | 13/15 | 10 of 12 | 0 | 0.000 |

Gordon set the levels on 2026-11-02, with best-in-class recall as the bar: `light` 4.0, `normal` 5.0, `strong` 6.0. `normal` meets the target, and every boost from 4.43 to 6.04 gives its 13/15.

## Consequences

- The boost has a measured ceiling. The nearest distractor, "hyper landing", becomes Hyprland at 6.41 nats per piece, so `normal` leaves 1.41 nats per piece before a false insertion on this set and `strong` 0.41. `normal` clears the last Hyprland clip, which needs 4.43, by 0.57. Both margins come from two synthetic voices, and a real speaker's near-misses will spread wider.
- Scoring the term in other casings and summing over every segmentation of it moved no clip, so neither was kept (ADR 0088).
- Rescoring added a median of 14 to 15 ms to the 29 clips with a candidate and at most 1.3 ms to a clip without one at every measured boost on the padded clips, preparing the vocabulary included, on the release build with Vulkan; the bound of 150 ms holds with room for a vocabulary of many terms.
- A future change that does move recall at the same boost would have to change the scores themselves, for example by a model fine-tuned on the user's terms. Revisit this record if such a model or a parakeet.cpp contextual-biasing API appears.
