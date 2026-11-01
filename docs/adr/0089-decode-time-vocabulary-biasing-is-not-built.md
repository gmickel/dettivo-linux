# 0089. Decode-time vocabulary biasing is not built, because it weighs the same scores rescoring already weighs

Status: Accepted 2026-11-01

## What this gives you

You know where Parakeet's vocabulary support stands and what would move it: rescoring after the decode (ADR 0088) brings back 5 of the 12 terms Parakeet misses on the benchmark at the default `normal` level and 8 at `strong`, with no term written where it was not said. A second biasing stage inside the decoder would not raise those numbers, so it is not built.

## Situation

fn-84 staged the work. Stage 1 rescores the greedy transcript (ADR 0088). Stage 2, decode-time biasing, was to be built only if stage 1 missed the recall target at `normal`: at least two thirds of the terms Parakeet misses recovered, with no false insertion and no rise in guard word error rate. Stage 2 carries shadow hypotheses beside the greedy path that spell a term through a token trie wherever its first piece is plausible at a word start, gives each piece the full boost when the greedy words sound like the term and 0.6 x the boost otherwise, and lets the term replace the greedy span at the word boundary only if it scores higher.

The benchmark ([vocabulary-2026-11-01-thor-rescoring](../reports/benchmarks/vocabulary-2026-11-01-thor-rescoring.md), Parakeet v3 on Vulkan at `2e1f0f4`, the GPU shared with other benchmark runs) measured stage 1:

| Level | Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER |
|---|---|---|---|---|---|
| off | 0 | 3/15 | 0 of 12 | 0 | 0.000 |
| light | 2.0 | 6/15 | 3 of 12 | 0 | 0.000 |
| normal | 2.5 | 8/15 | 5 of 12 | 0 | 0.000 |
| strong | 4.0 | 11/15 | 8 of 12 | 0 | 0.000 |

`normal` misses the two-thirds target (8 of 12). Every target clip produced a candidate, so matching is not what fails. The scores are: each missed term's forced score sat 2.3 to 8.8 nats short of the heard words after the `normal` boost (`dativo` for Dettivo 2.3, `O Markey` for Omarchy 3.7, `Hyperland` 5.9 and 7.1, `HyperLAN` 8.0, `DeDivo` 8.8). The distractors sat 24 to 47 nats short even at `strong`.

## Decision

- Stage 2 is not built. A shadow hypothesis is scored by the same prediction and joint networks over the same frames, with the same boost per piece or less. Stage 1 already sums every TDT alignment of the term over a window padded 4 frames on each side, which bounds what any single decode-time path through those frames can score. A hypothesis that starts at the same word boundary cannot beat that sum, and the gaps above are the model's own preference for a known spelling.
- The levels stay as specified. Recall at `normal` stays below the target and is recorded as missed. `strong` reaches the target on this set with no false insertion and is the documented setting for a vocabulary of coined names.

## Consequences

- Whether `normal` should add more than 2.5 nats per piece, or whether the default level should be `strong`, is a product call left open. The distractor margins (24 nats or more at `strong`) leave room, but the set holds five distractors and four guard sentences from two synthetic voices, so a larger margin of safety has not been measured.
- Scoring the term in other casings and summing over every segmentation of it moved no clip, so neither was kept (ADR 0088).
- Rescoring added a median of 15.0 ms to the 15 clips with a candidate and at most 1.4 ms to a clip without one, preparing the vocabulary included,, on the release build with Vulkan; the bound of 150 ms holds with room for a vocabulary of many terms.
- A future change that does move recall at the same boost would have to change the scores themselves, for example by a model fine-tuned on the user's terms. Revisit this record if such a model or a parakeet.cpp contextual-biasing API appears.
