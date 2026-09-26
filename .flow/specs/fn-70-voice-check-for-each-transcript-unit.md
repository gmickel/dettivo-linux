# Voice check for each transcript unit

## Goal & Context

After fn-68, every unit has a speaker by overlap. The errors that remain concentrate in short turns and turn edges. On AMI test, turns of three words or fewer were wrong 46% of the time with Nemotron and 60% with pyannote, against under 3% in single-speaker speech (noScribe issue #360).

noScribe PR #351 attacks exactly this. It embeds each sentence or pause unit and compares it with each speaker's centroid. It moves the unit to a closer speaker at any margin when the diarization agrees, or at a calibrated cosine margin (0.12, scaled per recording) when only the voice points there. On unseen AMI, CallHome and VoxConverse audio, wrong-speaker words fell from 5.3% to 2.4%. The PR also records what failed, which this spec should not repeat. Details are in the vault note "dettivo-linux -- Speaker attribution research (2026-09-26)".

## Approach

- Speaker centroids come from each speaker's confident units. Each unit's embedding comes from an embedding model the product already runs (ERes2Net), or from WeSpeaker ResNet293-LM if the bench shows it is better. Evaluate the multilingual option for German.
- Move a unit only under the PR #351-style margins, calibrated per recording. Tune the parameters on the fn-67 bench's AMI dev set, never on test.
- When the engine provides frame probabilities (Nemotron, fn-69), the bench also evaluates them as the evidence in place of, or alongside, embeddings, and keeps whichever wins.
- The check is a step inside the speaker pass, switchable in `config.toml`.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** On the fn-67 bench, the voice check lowers the headline attribution error on AMI dev and on the local meetings for both engines, relative to fn-68 alone. The bench reports words fixed against words broken.
- **R2:** Short-turn error (turns of three words or fewer) falls on AMI dev, and the `--heldout` test figures confirm it without retuning.
- **R3:** German shows no regression, reported separately.
- **R4:** The added time per meeting hour is reported for CPU and CUDA, and stays within the speaker pass's current time budget, or the report states the cost.
- **R5:** A config key switches the check off, and it is documented. An ADR records the method and its measured effect.
- **R6:** Unit tests cover the margin logic, including the "diarization agrees" and "voice only" branches and per-recording scaling.

## Boundaries

- No LLM correction.
- No per-word reassignment and no neighbour smoothing: PR #351 measured both as net-harmful.
- No stored voice identities across meetings here (fn-71 covers the user's own voice).

## Decision Context

Gordon approved recommendation 3 on 2026-09-26.
