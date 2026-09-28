# 0076. Each sentence is checked against the speakers' voices, and moves when its voice clearly belongs to another speaker

Status: Accepted 2026-09-28, amends [0072](0072-every-remote-line-takes-the-speaker-of-its-sentence.md) (a sentence's vote can now be overruled by its voice)

## What this gives you

Fewer lines carry the wrong name. With the current engine, AMI dev words under the wrong speaker fall from 21.07% to 18.60%, and the held-out AMI test words fall from 14.11% to 11.85%. With Nemotron they fall from 12.49% to 11.98% on AMI dev and from 11.96% to 11.52% on AMI test. The check costs about 25 seconds per hour of meeting on the CPU, and `[meetings.diarization] voice_check = false` turns it off.

## Situation

After ADR 0072 and ADR 0074, every sentence unit takes the speaker holding most of its time. The errors that remain sit in short turns. On AMI dev, words in turns of three words or fewer were wrong 61.7% of the time with the current engine and 52.2% with Nemotron. Labelled lines with the wrong speaker were 21.0% and 11.2%. The engine's turns are the only evidence the rule reads, so a sentence the engine gave to the wrong person has nothing to correct it.

noScribe PR #351 adds a second opinion. It embeds each sentence or pause unit and compares it by cosine with each speaker's centroid. A unit moves to a closer speaker at any margin when the diarization inside it also names that speaker, and by a margin of 0.12 when the voice alone points there. Both margins shrink with the recording's median own-speaker margin. On unseen AMI, CallHome and VoxConverse audio it cut wrong-speaker words from 5.3% to 2.4%. The PR also measured per-word assignment, cuts at every diarized change and smoothing over neighbouring units as net-harmful, so none of them is tried here.

The product already ships an embedding model, the sherpa-onnx set's ERes2Net, and ADR 0075 gave `dettivo-engine-diarize` an `embed` request. Nemotron has no embedding model, but it keeps per-10 ms speaker probabilities, which can stand in for the voice.

## Decision

- **Where it runs.** The check is a step of the sentence rule (`crates/dettivo-meeting/src/voice_check.rs`, called from `sentences::label`). It runs after each unit's vote and before a segment splits into speaker runs, so a moved sentence becomes its own stored segment like any other split. It sees only the units the rule assigns: every line in room audio, the system track's lines in a two-track meeting. It runs before the single-remote shortcut and the voiceprint of ADR 0075.
- **The voice.** The pass embeds every labelled unit of at least 500 ms on the diarized track, in one `embed` call, with the same engine and model set that embeds the voiceprint (ADR 0075). Under Nemotron that is the `diarization-en` set. Without it, the check and the voiceprint are skipped for that meeting, and the meeting is labelled as before.
- **The centroids.** A speaker's centroid is the mean voice of that speaker's confident units: at least 1.5 seconds long, with no other speaker's turn inside. A speaker with no confident unit uses all its units.
- **The move.** A unit's current speaker is its vote. When another speaker's turns also lie inside the unit, the unit moves to the one of them holding the most time if the voice favours it by more than `agree` (0.15). Then it moves to the closest voice if that voice wins by more than `overrule` (0.2). Both margins are multiplied by the recording's scale: the median margin by which confident units favour their own speaker, over 0.5, kept between 0.5 and 1. A unit under 500 ms never moves. A moved unit's `speaker_confidence` is its new speaker's share of its diarized time, 0 when the voice alone moved it.
- **Configuration.** `[meetings.diarization] voice_check` (true) turns it on and off. The margins are constants: the bench showed a flat optimum and no reason to hand them to users.
- **Evidence kept.** Embeddings. Nemotron's frame probabilities, scored as each speaker's mean probability over the unit, moved AMI dev by -0.03 points [-0.30, +0.32] and are not used. WeSpeaker ResNet293-LM is not evaluated: the embeddings separate AMI speakers well (below), so the model is not the limit.

## Consequences

The fn-67 bench measured the check against its own rule with `voice_check false` on the same cached inputs, with Whisper's word timings of ADR 0074. The check-off run reproduced the ADR 0074 scoreboard exactly (AMI dev 21.07% and 12.49%). The margins were tuned on AMI dev only, and AMI test was scored once afterwards. The report is [diarization-bench-2026-09-28-voice-check](../reports/benchmarks/diarization-bench-2026-09-28-voice-check.md). Its tree hash is the commit that keeps units no turn overlaps out of the confident set; the table below was re-measured there on the same cached embeddings. The margin sweep, the frame-probability comparison and the 2-means figures below were measured before that fix.

| Split, metric | Current engine, off → on | Nemotron, off → on |
|---|---:|---:|
| AMI dev, attribution error | 21.07% → 18.60% (-2.47 [-3.19, -1.71]) | 12.49% → 11.98% (-0.50 [-1.14, -0.17]) |
| AMI dev, short-turn error | 61.70% → 56.86% (-4.84 [-6.21, -3.34]) | 52.21% → 51.41% (-0.80 [-1.10, -0.54]) |
| AMI dev, words fixed / broken | 3.90% / 1.41% | 1.57% / 1.05% |
| AMI dev, labelled lines with the wrong speaker | 21.02% → 19.53% | 11.24% → 11.88% |
| AMI test (held out), attribution error | 14.11% → 11.85% (-2.26 [-3.39, -0.71]) | 11.96% → 11.52% (-0.44 [-0.63, -0.09]) |
| AMI test (held out), short-turn error | 60.85% → 57.46% (-3.39 [-4.87, -1.01]) | 54.87% → 54.30% (-0.57 [-1.77, +0.17]) |
| AMI test (held out), words fixed / broken | 3.77% / 1.50% | 1.25% / 0.80% |
| Retained meetings, units moved | 1.02% (English 1.27%, German 0.84%) | 0.51% (English 0.73%, German 0.37%) |

- The gain is mostly the current engine's. Nemotron already confuses few speakers (0.36% engine confusion on AMI dev), so the check has less to fix, and on AMI test its short-turn interval spans zero. With Nemotron the check splits more lines, and the line-count view of wrong names rises by 0.64 points on AMI dev while the word-weighted headline falls.
- The margins barely matter. On AMI dev every pair tried from agree 0 to 0.15 and overrule 0.12 to 0.3 lands within 0.35 points of the others for the current engine and 0.15 for Nemotron, inside every interval. The chosen pair breaks the fewest words: noScribe's pair (0, 0.12) broke 1.90% and 1.67%.
- The retained meetings carry no speaker labels, so the bench has no headline for them and cannot say whether the moves there are right. The check moves under 2% of their units, fewer in German than in English, and changes no proxy: blank remote lines, "You" lines on remote-only speech and local speech outside "You" lines are identical with the check on and off. A German regression cannot be ruled out until fn-72's labels exist. noScribe measured German CallHome calls improving from 2,838 to 1,754 wrong words with the same method.
- The cost is one more embedding call per meeting. On this machine a release build of the engine embeds 1,484 units from 1.44 hours of AMI audio in 24 seconds per hour on the CPU with four threads, and in 33 seconds per hour on CUDA (RTX 4090, shared with another GPU program during the run). The bench's debug build ran at about 100 to 110 times realtime. The speaker pass itself takes about 3 to 4.5 minutes per hour with the current engine on the CPU and about 70 seconds with Nemotron on the CPU, so the check adds about a tenth to the former and a third to the latter. No time budget for the pass is written down, so this states the cost instead.
- Two voices merged into one cluster are visible in the voices on AMI. Splitting a cluster's confident units in two by 2-means leaves centroids at a median cosine of 0.78 when the units are one reference speaker's and 0.17 when they are two speakers' (AUC 0.94 over 70 speakers and 102 pairs on AMI dev). ADR 0075 found no such separation on the retained meetings' lines, so the single-remote shortcut keeps relying on the engine alone. A detector is left for a later spec.
- The bench asks the assignment stage for each job's unit spans (`mode: spans`), embeds them on the diarized track with the tree's engine, caches the vectors under `cache/embed/`, and scores the labelling with and without the check to count the words fixed and broken.
