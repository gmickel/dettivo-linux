# 0075. Two-track meetings use both tracks to name speakers

Status: Accepted 2026-09-27, amends 0035 and 0072

## What this gives you

A meeting's "You" lines stop carrying words you never said. The speaker pass drops a microphone line when your microphone was barely voiced during it, which cuts the time "You" lines spend on remote-only speech from 37.9% to 22.0% on the retained meetings. A call with one remote person gets that person on every remote line. A meeting recorded around one shared microphone is labelled like room audio. The pass also learns your voice from your own microphone on this machine, so a remote line in your voice is named You.

## Situation

Dettivo records the microphone and the system track separately and diarizes only the system track (ADR 0035). The microphone's lines were all "You", whatever they held. On the fn-67 bench's ten retained meetings, 37.9% of the single-side speech under "You" lines was remote-only speech: frames where the system track spoke and the microphone stayed below -60 dBFS. Much of it came from Whisper hearing words in silence on the microphone. The largest cases are 30-second lines holding two or three words over a microphone that never rose above -45 dBFS. With headphones, the usual setup (fn-65), little real bleed reaches the microphone.

The open recorders studied for this spec add four rules on top of the same two-track design:

- omarchy-meeting-recorder keeps a microphone frame only when it is at least half as loud as the system audio nearby. It also drops "You" sentences whose word trigrams repeat remote text.
- OpenWhispr drops microphone segments that match system text within 6 seconds.
- anarlog labels the system track directly when it holds one remote voice, and diarizes the microphone when the system track is silent while several people speak.
- Every meeting's microphone is free enrolment audio for the user's voiceprint.

Each rule was measured on the fn-67 bench (`just diar-bench`). The baseline had every rule off, and the bench reproduced the saved pre-change scoreboard exactly. Two new bench metrics come from the local/remote proxy frames. The first is "You lines on remote-only speech", the share of the single-side frames under "You" lines that are remote-only. The second is "Local speech outside You lines", the local-only speech no "You" line holds, which is the cost of a gate that drops too much. AMI is room audio, so none of these rules can touch it. No retained meeting has a labels file yet, so the local meetings have proxies and no headline.

| Rule, alone | You lines on remote-only speech | Local speech outside You lines | Other effect |
|---|---|---|---|
| All rules off | 37.86% | 3.07% | |
| Level gate, 0.02 of the line voiced | 27.14% (-10.72 [-15.30, -5.07]) | 3.13% (+0.06 [+0.02, +0.09]) | 12.7% of microphone lines dropped |
| Level gate, 0.05 (default) | 22.00% (-15.86 [-21.83, -9.30]) | 3.39% (+0.32 [+0.21, +0.41]) | 15.2% dropped |
| Level gate, 0.10 | 16.13% (-21.73 [-28.52, -14.17]) | 4.17% (+1.09 [+0.77, +1.41]) | 18.7% dropped |
| Text gate (half the trigrams within 6 s) | 37.86% (-0.00) | 3.14% (+0.07 [+0.00, +0.16]) | 0.11% dropped |
| Single remote | unchanged | unchanged | Fires on 2 of 10 meetings; Nemotron's blank remote lines 2.40% to 2.23% |
| Voiceprint | unchanged | unchanged | 0 remote lines relabelled |
| Shared microphone | unchanged | unchanged | 0 of 10 meetings switched |

The deltas are paired 95% bootstrap intervals over the ten meetings, and they are the same with both engines because the rules act on the microphone's lines. With every kept rule on, AMI dev attribution error stays 22.43% with the current engine and 14.65% with Nemotron (+0.00 for both). AMI test is 16.05% and 14.70% (the [report](../reports/benchmarks/diarization-bench-2026-09-27-two-track.md)). Those AMI runs used the bench's cached Whisper transcripts, which predate the word timings of ADR 0074, and they were not recomputed. The rules skip room audio, so recomputing would move the absolute AMI figures and leave the deltas at zero.

Embeddings come from the ERes2Net model the diarization engine already ships. Your own microphone lines sit at a median cosine similarity of 0.73 to that meeting's voiceprint, with the 10th percentile between 0.36 and 0.57. No remote line on any retained meeting reaches 0.5: the highest is 0.47. The single-remote shortcut was meant to need both the engine and a voice check. As a voice check, a two-way split of a track's line embeddings separates one voice's microphone lines as far apart (cosine 0.05 to 0.77 between the halves) as the system tracks with several voices (0.04 to 0.52). The check cannot tell one voice from two. Merging the engine's clusters by centroid similarity never fired, and the closest pair of distinct clusters sat at 0.53.

## Decision

The speaker pass of a two-track meeting (`dettivo_meeting::two_track`) applies these rules around the sentence rule:

1. **Bleed gate.** A microphone frame is voiced when the microphone is above -45 dBFS and at least half as loud (within 6 dB) as the system track. The pass drops a microphone line voiced for less than `bleed_min_voiced` (0.05) of its time, and rebuilds the transcript text without it.
2. **Single remote.** When the engine finds exactly one remote speaker, every remote line takes that speaker, including the lines no turn lies near (`single_remote`).
3. **Shared microphone.** When at most 0.5% of the system track is active while at least 5% of the microphone is, the pass diarizes the microphone instead (`shared_mic`). Two or more voices there label every line like room audio. One voice keeps the lines as You.
4. **Your voice.** The pass embeds every line of at least 1.5 seconds on its own track. The microphone lines voiced for at least half their time give the meeting's voiceprint, which needs at least three of them. That voiceprint is averaged with the stored one. A remote line within `voice_match` (0.6) cosine similarity of the result becomes You. The stored voiceprint is one 192-value vector in `<data_dir>/voiceprint.json` (mode 600), keyed to the diarization model set. It carries more weight than any one new meeting, up to 20 to 1, and remembers the last 200 meetings it folded so a re-run counts a meeting once. `voiceprint = false` stops the pass from reading or writing it. The embeddings come from a new `embed` request on `dettivo-engine-diarize`: spans in, one unit vector per span out, computed in the engine process on this machine.

The text gate is not shipped. It dropped six lines on the retained meetings, moved no remote-only speech, and the frames it removed were local speech by the proxy. Echo cancellation (WebRTC AEC3) is not needed while the level gate meets R1. The single-remote shortcut rests on the engine alone, because no voice check tried here could tell one voice from two.

## Consequences

- The pass reads both tracks of a two-track meeting, roughly 230 MB of samples for an hour, and it makes two `embed` calls when the voiceprint rule is on. An engine older than this change answers `embed` with `bad_request`. The pass then logs it and runs without the voice rule.
- A dropped line leaves the stored transcript and its polished text. The automatic analysis starts after the pass, so it reads the transcript without them.
- The gate drops about 0.3% of the local speech, the local-only frames inside the lines it removes. `bleed_min_voiced = 0.02` keeps nearly all of it and still removes two thirds of the leak (27.1%).
- The voiceprint never leaves the machine, and no network path reads it. Deleting the file forgets it.
- Naming yourself in room audio, where the voiceprint could pick your cluster, is left for a later spec. The same goes for a labelled local set that would give the local meetings a headline.
- The bench runs the tree's engine for embeddings (`--embed`), caches them per meeting under `cache/embed/`, and passes each meeting's directory to the assignment stage, which reads both tracks' levels with the product's own reader.
