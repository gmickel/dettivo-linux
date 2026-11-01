# Vocabulary boost levels as dictation hears them, 2026-11-01 on thor

This run measures Parakeet's vocabulary boosts on clips padded the way dictation pads a take. Padded, `normal` (4.0 nats per piece) recovers 7 of the 12 terms Parakeet misses, one short of the two-thirds target in R2. It writes no term where none was said. The three Hyprland clips need 4.2 to 4.4 nats per piece, and at 4.5 to 6.0 all three come back with no false insertion. This report replaces the recall figures in [vocabulary-2026-11-01-thor-boosts](vocabulary-2026-11-01-thor-boosts.md), which were measured on unpadded clips. Gordon set the levels from it on 2026-11-02: `light` 4.0, `normal` 5.0, `strong` 6.0 nats per piece. A run of the shipped `normal`, below, confirms the 5.0 row.

## What changed in the bench

Dictation never sends a take to the engine as captured. `pad_for_engine` makes it at least 1.5 s long and adds 400 ms of trailing silence for Parakeet (fn-79). The bench used to send each fixture clip as it is. That moved the rescoring margins enough to matter: unpadded, `target-hyprland-1` cleared the 4.0 boost by 0.10 nats, and padded it misses by 0.78. QA found the gap through the daemon, which gave 10/15 at `normal` where the unpadded bench gave 11/15. The bench now pads each clip with the session's own function (`ad49316`), so its numbers are the ones a user gets.

## Results

Parakeet v3 (`tdt-0.6b-v3-q8_0`), release build, Vulkan, engine at `c5ef6e3` with the bench padding of `ad49316`, on the 47-clip set (`set.json` `1f7c1acafdde`). The bench runs its `on` pass at `normal`, so each boost ran from its own engine build with `normal` mapped to that boost. The four runs are filed as `vocabulary-2026-11-01-thor-padded-boost-<boost>.json`. Other agents were using the GPU (38 % busy at the start).

| Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER | Rescoring added |
|---|---|---|---|---|---|
| off | 3/15 | 0 of 12 | 0 | 0.000 | - |
| 2.5 | 8/15 | 5 of 12 | 0 | 0.000 | 13.8 ms median on 29 clips, 1.3 ms most without |
| 4.0 | 10/15 | 7 of 12 | 0 | 0.000 | 14.5 ms median on 29 clips, 1.3 ms most without |
| 5.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.5 ms median on 29 clips, 1.2 ms most without |
| 6.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.8 ms median on 29 clips, 1.1 ms most without |

All four off passes scored the same: 3/15, no insertion, guard WER 0.000. Against the unpadded run, 4.0 loses `target-hyprland-1` and 6.0 loses its false insertion: padded, "The kids were hyper landing" stays as said.

## Where each clip crosses

A candidate replaces the heard words when `log P(term) + boost x pieces >= log P(heard)`, so each candidate has one crossing boost, `(log P(heard) - log P(term)) / pieces`. One debug pass of the 4.0 engine over the padded clips gives every crossing, and they agree with all four measured rows.

| Clip | Heard (off) | Pieces | Crossing boost |
|---|---|---|---|
| target-sapienxt-2 | Sapien XT | 5 | 0.67 |
| target-sapienxt-3 | Sapien XT | 5 | 1.63 |
| target-omarchy-3 | Omar He | 4 | 1.85 |
| target-sapienxt-1 | Zapian XT | 5 | 1.86 |
| target-omarchy-1 | Omarkey | 4 | 2.36 |
| target-omarchy-2 | O Markey | 4 | 3.39 |
| target-dettivo-1 | dativo | 2 | 3.50 |
| target-hyprland-1 | Hyperland | 4 | 4.20 |
| target-hyprland-2 | Hyperland | 4 | 4.32 |
| target-hyprland-3 | Hyperland | 4 | 4.43 |
| target-dettivo-2 | Itivo | 2 | 6.04 |
| distractor-hyper-landing | hyper landing | 4 | 6.41 |
| target-dettivo-3 | DeDivo | 2 | 7.03 |
| distractor-omar-he | Omar, he | 4 | 8.32 |
| distractor-sapiens-next | Sapiens next | 5 | 9.11 |
| distractor-anarchy | anarchy | 4 | 9.45 |

The other 13 distractors with a candidate cross at 9.98 or above, and the seven guards produce no candidate.

## What the boost buys

- **Two thirds (8 of 12) needs at least 4.20.** Below it the Hyprland clips stay Hyperland. At 4.0, `target-hyprland-1` is 0.78 nats short.
- **The most recall with no false insertion is 14/15 (11 of 12), and only between 6.04 and 6.41.** That window is 0.37 nats per piece wide, 1.5 nats on "hyper landing". The pad alone moved that distractor by 0.6 nats per piece (unpadded it crossed at 5.8), so this window is too narrow to ship.
- **5.0 recovers 10 of 12 with room on both sides.** The last Hyprland clip clears it by 0.57 nats per piece (2.3 nats). The nearest distractor, "hyper landing", stays 1.41 nats per piece short (5.7 nats). Every boost from 4.43 to 6.04 gives the same 13/15.
- **4.5 is the lowest boost that holds all three Hyprland clips**, with 0.07 nats per piece (0.3 nats) to spare on `target-hyprland-3` and 1.91 nats per piece (7.7 nats) below "hyper landing".
- `DeDivo` for Dettivo needs 7.03, past "hyper landing", so no boost recovers all 12 without a false insertion on this set.

The set comes from two synthetic voices. A real speaker's near-misses will spread wider than these crossings, so the headroom below "hyper landing" matters as much as the recall.

## The shipped levels

The levels are `light` 4.0, `normal` 5.0 (default) and `strong` 6.0 nats per piece (`ec31f13`). One run of the bench measured the shipped `normal` from an engine built as it ships, with no level remapped: the same release Vulkan build, the same padded clips and set (`1f7c1acafdde`), at `ec31f13`. It is filed as `vocabulary-2026-11-01-thor-padded-normal.json`. The bench always runs its `on` pass at `normal`, so the `light` and `strong` rows are the 4.0 and 6.0 rows above.

| Level | Boost (nats per piece) | Term recall | Missed terms recovered | False insertions | Guard WER | Rescoring added |
|---|---|---|---|---|---|---|
| off | 0 | 3/15 | 0 of 12 | 0 | 0.000 | - |
| `light` | 4.0 | 10/15 | 7 of 12 | 0 | 0.000 | 14.5 ms median on 29 clips, 1.3 ms most without (row above) |
| `normal`, shipped build | 5.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.3 ms median on 29 clips, 1.3 ms most without |
| `strong` | 6.0 | 13/15 | 10 of 12 | 0 | 0.000 | 14.8 ms median on 29 clips, 1.1 ms most without (row above) |

The shipped run matches the remapped 5.0 build clip for clip. The two terms it misses are `Itivo` and `DeDivo` for Dettivo, and "The kids were hyper landing after the long flight" stays as said. `normal` sits 0.57 nats per piece above the last Hyprland clip and 1.41 below "hyper landing"; `strong` sits 0.41 below it. The run also measured Whisper `large-v3-turbo` on the CPU, which an engine directory without a Whisper build skips; those rows are not part of this comparison. Other agents were using the GPU (20 % to 40 % busy).
