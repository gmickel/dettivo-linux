# 0102. Dictation decodes in rounds while the key is held

Status: Accepted 2026-11-07. Amends 0094.

## What this gives you

A long Parakeet dictation on a GPU no longer waits for the whole take to be transcribed after you let go. While you hold the key, the daemon re-transcribes what you have said so far every 1.5 s, and at the release it transcribes only the last few seconds and joins them to the rest by word timestamps. On the reference GPU a two-minute take shows its text 273 ms after the release at p50 instead of 841 ms, a one-minute take 114 ms instead of 525 ms, and a 30 s take 88 ms instead of 349 ms. The words come out as accurate as a transcription of the whole take: on the FLEURS takes, Parakeet Ultra and v3 in rounds score at or below one whole transcription, in English and in German.

## Situation

Release-to-text grew with the take: 489 ms at p50 for 30 s, 583 ms for 60 s and 890 ms for 120 s on the reference GPU with Parakeet Ultra (ADR 0097). Most of that growth is the offline decode of the whole take after the release. ADR 0094 found that parakeet.cpp cannot stream the v2, v3 or Ultra dictation models and set aside a buffered decoder with 5.6 s of left context.

An offline model can still do most of its work during the hold. Published measurements of re-decoding with a long left context, committing the words well behind the newest audio and decoding only the tail at the release found Parakeet TDT's word error rate unchanged against a whole-file decode on 1,351 public clips (+0.0009, 95% CI -0.0006 to +0.0023), and a 4 s left context cost +0.0119. Short windows, ADR 0094's buffered decoder and the meeting path's 3 s windows (ADR 0031), are the wrong shape for dictation.

## Decision

`dettivo_transcribe::rounds` holds the scheme and `crates/dettivo-session/src/worker/held.rs` runs it during the take.

- **Who gets rounds.** Parakeet on a GPU backend, once the press qualifies as a take (ADR 0093). A CPU backend, Whisper and a take shorter than one cadence keep the whole-take decode unchanged.
- **Rounds.** A `dictation-rounds` thread keeps a copy of the take as the recording loop sends it chunks; the loop never waits on it. Every 1.5 s it decodes from 20 s before the commit point to the newest audio, padded as dictation pads a take, with the take's own request (language, vocabulary). A round that overruns skips the rounds it missed. Parakeet is dictation-only (ADR 0018), so no meeting shares its process, and a meeting's capture and a dictation never run at once.
- **Commits.** Once a round covers 60 s of the take, the words that end 20 s or more behind its end are committed. A word belongs to the side of a cut its centre falls on, and a cut sits in the middle of the pause between two words.
- **The release.** No round starts after the release; the release waits for the round already running, since the engine is one process. When the speech check says the take's last voiced window ended before the last round's end, that round's words are the text and nothing is decoded. Otherwise the release picks a cut in the last round's words near 4 s before the take's end, at least 2 s before the round's end, preferring the longest pause within 2 s of that target, and decodes from 2 s before the cut to the end. When the take kept its whole release tail, or the speech check already said where its voice ended and that is after the last round's end, the decode starts once the take's samples are final, while the capture's stop is acknowledged, the take file is written and the speech gate judges the take; its words are dropped when the gate finds silence or the take ended at another length. A take that may still reuse its last round starts no decode before the gate. A padded tail or round that comes back empty runs once more as captured, as a whole take does (ADR 0081). The rounds' words before the cut and the release decode's words after it are the take's words, grouped into segments at pauses over 700 ms and every 30 s as the Parakeet engine groups a whole decode's words, and the output guards and the text step run on them as before.
- **Fallback.** A take whose last round ended more than 20 s before the release, or after the end of the take (a release that waited behind a slow start cut the take earlier), or whose rounds all failed, is decoded whole as before, and so is a stitched text that comes out empty. History records the path a take with rounds took in `timings.release_decode` (`tail`, `silence`, `whole`); a take without rounds records none.

## Consequences

The measurements are in [the benchmark report](../reports/benchmarks/dictation-rounds-2026-11-07.md), which lists the method and every number below.

### The chosen values

| Value | Chosen | Why |
| --- | --- | --- |
| Cadence | 1.5 s | Inside the 1-2 s the spec started from. The engine spent 4.8-5.2% of a 30 s hold, 8.0-8.4% of a 60 s hold and 9.2-9.4% of a 120 s hold on rounds, and no round was skipped. |
| Horizon and context | 20 s each, commits from 60 s | The published scheme's values. With them every pooled interval below lies under 0. |
| Release window | 4 s, plus 2 s of overlap | Within 0.0016 of the published 8 s window on every pooled WER row, and 2 to 3 s less audio to decode after the release. |

### Release-to-text (R1)

Release builds with Parakeet Ultra on Vulkan, measured through the daemon by the method of ADR 0094 and ADR 0097 on 2026-11-07, with other agents sharing the GPU.

| Take | Before (`791554d`), p50 (min-max) | After, p50 (min-max) | `transcribe_ms` p50, before and after |
| --- | --- | --- | --- |
| 30 s | 349 (306-775) ms | 88 (87-90) ms | 290, 26 |
| 60 s | 525 (449-649) ms | 114 (113-115) ms | 442, 30 |
| 120 s | 841 (796-1,251) ms | 273 (273-283) ms | 619, 27 |

- R1 asks for a p50 of at most 300 ms and a p90 no higher than ADR 0097's p50 (489, 583 and 890 ms). With five takes per length the nearest-rank p90 is the slowest take, 90, 115 and 283 ms.
- The 120 s takes end mid-word and keep the whole 200 ms release tail (`capture_ms` 225), so most of their 273 ms is the tail. What follows the tail no longer grows with the take: the gate and the tail decode take 26-30 ms at every length.
- The first build decoded an 8 s window and started it after the gate; its 120 s p50 was 313 ms. Starting the tail decode once the take's samples are final, before the capture's stop is acknowledged and the take file is written, and shortening the window to 4 s brought it to 273 ms.

### Word error rate (R2)

`dettivo-qa bench windowed` joined the first 400 FLEURS test utterances of each language into takes of 30, 60 and 120 s and transcribed each whole and in rounds. Over all three lengths, with the 4 s window and a 95% paired bootstrap over takes:

| Model | English: whole, rounds, difference (95% CI) | German: whole, rounds, difference (95% CI) |
| --- | --- | --- |
| Parakeet Ultra | 0.0571, 0.0552, -0.0019 (-0.0035 to -0.0005) | 0.0469, 0.0453, -0.0016 (-0.0028 to -0.0004) |
| Parakeet v3 | 0.2213, 0.1342, -0.0871 (-0.1079 to -0.0681) | 0.0938, 0.0726, -0.0212 (-0.0330 to -0.0104) |

- Every upper bound is below R2's +0.004. Each utterance appears once per take length, so the pooled interval counts it three times and is narrower than three independent sets would give.
- Per take length, every upper bound is at or below +0.0017 except Parakeet v3 in German at 30 s: +0.0021 (-0.0015 to +0.0068). Takes that short commit nothing, so the difference there comes from the release window alone.
- Parakeet v3 loses many words when it transcribes these joined takes in one pass (0.2616 whole against Ultra's 0.0608 on the English 120 s takes), and rounds recover most of them. The run does not establish why.
- The bench decodes every tail. The speech check's shortcut, which reuses the last round when only silence followed it, is not scored.
- The WER run predates the review fixes: the second pass of an empty padded round or tail, and segments rebuilt at pauses before the output guards. None of the run's whole decodes came back empty. The bench scores the raw stitched text, so the segments do not enter it, and a second pass replaces only a result that came back empty.
- The release-to-text run predates the same fixes. They leave its measured path as it was (inferred from the code and the run's logs): every round ran, no padded tail came back empty, the 30 s and 60 s takes ended with a voiced window after the last round's end and the 120 s takes kept their whole tail, so each still started its tail decode early.

### Whisper (R5)

Rounds do not ship for Whisper. With the same scheme, Whisper large-v3-turbo scored +0.0349 (-0.0037 to +0.0755) worse in rounds over the first 100 English utterances, +0.1001 on the seven 120 s takes alone, which breaks R2's bound. German, over 38 takes of 30 s and 22 of 60 s before the run was stopped to free the GPU, scored -0.0017 (-0.0052 to +0.0019). Whisper is the default engine (ADR 0085), and its takes keep the whole-take decode until a scheme passes the bound for it.

### Other consequences

- A take the rounds ran on decodes about 5 to 8 s at the release, so release-to-text after the tail stays near 30 ms however long the take.
- The engine runs during every Parakeet take on a GPU, about 5 to 10% of the hold on the reference GPU. A take on the CPU backend runs no rounds, and neither does a take shorter than 1.5 s.
- History stores `timings.release_decode` for a take with rounds; older rows and takes without rounds carry none, and the field is optional on read.
- `crates/dettivo-session/tests/rounds.rs` feeds 12 s of audio while each round takes 700 ms: capture never waited, every round decoded the take from its start with every sample captured by then, the release decoded only the tail, and the text holds every second once in order (R4). The same file decodes a take on the CPU, a take shorter than one cadence and a take whose rounds all failed, each in one whole request (R3).
