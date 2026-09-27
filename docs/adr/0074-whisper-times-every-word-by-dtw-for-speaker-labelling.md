# 0074. Whisper times every word by DTW, and a segment's span follows its words

Status: Accepted 2026-09-27, amends [0072](0072-every-remote-line-takes-the-speaker-of-its-sentence.md) (its note on word timings)

## What this gives you

Every Whisper segment in a meeting, an import or a re-run now carries its words with a start, an end and a confidence, and its start and end come from those words. On AMI dev the speaker pass then misattributes 21.07% of words with the current engine, down from 22.43%, and 12.49% with Nemotron, down from 14.65%, and the held-out AMI test split agrees. Named lines with the wrong speaker do not come back down: they stay at 21.02% and 11.24%, where ADR 0072 left them.

## Situation

ADR 0072 left one route to fewer wrong names. Most lines the old rule left blank hold two voices, and a line can only split between them where the transcript knows when each word was said. The Whisper engine returned `words: Vec::new()` on every segment, so the pause cut of ADR 0072 (`pause_ms`) never fired on a Whisper transcript.

whisper.cpp offers two sources of token times, and whisper-rs 0.16 exposes both. Plain token timestamps (`token_timestamps`) spread each segment's span over its tokens. DTW token timestamps align the decoder's cross-attention to the audio over the model's alignment heads, and whisper.cpp ships a head preset for every model in Dettivo's catalogue, from tiny to large-v3-turbo. DTW runs inside the decode on the loaded backend and gives each token one time on a 20 ms grid.

Measured on three AMI dev meetings (5,549 Whisper words paired with the AMI manual word times) and on the jfk fixture against its golden alignment (ADR 0018), with large-v3-turbo:

| Word times from | AMI start, median / p95 | AMI end, median / p95 | AMI pooled | jfk pooled |
|---|---:|---:|---:|---:|
| Plain token timestamps | 430 / 2,620 ms | 410 / 2,330 ms | 420 / 2,480 ms | 206 / 592 ms |
| DTW, a word ending where the next one starts, inside its segment | 180 / 540 ms | 190 / 1,070 ms | 180 / 820 ms | 230 / 1,167 ms |
| DTW, a word ending 20 ms after its last token, never past the next start (chosen) | 170 / 440 ms | 40 / 720 ms | 120 / 540 ms | 102 / 437 ms |

The plain times are poor because they inherit whisper's segment timestamps, and in long audio those drift by several seconds. On IS1008a whisper places "So who is marketing?" at 111.0 to 112.0 s. The AMI reference has it at 115.7 to 117.0 s and DTW at 115.9 to 117.1 s. Between consecutive paired words, plain timing showed a gap of 250 ms or more at 8 of the 334 places where the reference has such a pause or a change of speaker, against 175 with the chosen DTW rule. A plain-only run of IS1008a, with no DTW heads loaded, gave the same plain figures, so the DTW context does not disturb them.

## Decision

- **DTW with the model's preset.** `crates/dettivo-engine-whisper/src/words.rs` reads the ggml header (vocabulary, layer counts, mel bins) and picks the alignment-head preset for tiny, base, small and medium in both English and multilingual forms, large-v3 and large-v3-turbo. Large-v1 and v2 share one shape and get no preset. A model without one falls back to plain token timestamps. Flash attention stays off, since whisper.cpp disables DTW under it.
- **Words.** A word is a run of non-whitespace token bytes, so sub-word tokens and trailing punctuation merge into the word they continue and a segment's words are its text split on whitespace. A word starts at its first token's DTW time and ends one 20 ms frame after its last token's, never past the next word's start. Its confidence is the mean probability of its tokens.
- **Spans follow the words.** A segment's start and end snap to its first and last word, since the words are the better clock. The chunked merger already snapped a merged segment to its words, so a meeting's segments and a whole-file CLI run now agree on the rule.
- **No segment closes before 160 ms under DTW.** whisper.cpp 1.8.3 median-filters the DTW alignment over seven 20 ms steps and aborts the process when a decode advanced seven steps or fewer, which a segment closed within 160 ms of its window's start produces (the `filter_width` assertion in `median_filter`). The fix is not upstream. The engine's logits filter masks the timestamps before 160 ms once a decode holds a timestamp, and leaves the opening timestamp free. Without it, one of the 22 AMI recordings and the longest retained meeting (chunk 348 of 555, twice, so the finalisation failed) aborted the engine on the RTX 4090. With it both finish.
- **No words under VAD.** whisper.cpp maps segment times out of VAD-compressed audio but not token times, so a load with a VAD model returns segments without words. The daemon loads no VAD model.
- **Everywhere timestamps are asked for.** Meetings, imports and re-runs store the words through the contract's `Segment.words`. Dictation asks for timestamps too and gets the words for free: the transcript is identical and the jfk clip took 15.1 s with the words and 15.1 s without, load included, on the same backend.
- **`pause_ms` is 500.** Tuned on AMI dev with the words in place (below), it replaces ADR 0072's 250 in `[meetings.diarization]`, `config.keys` and the Settings route.

## Consequences

The fn-67 bench scored the rule against ADR 0072's figures (baseline `fn74-before`, the sentence-units report). Every Whisper result came from this engine on the RTX 4090. AMI went through the engine's CLI mode and the ten retained meetings through the product's finalisation (`"asr": {"meetings": true}`, `crates/dettivo-meeting/examples/meeting_asr.rs`), so the meeting figures read what a finished meeting would now store. The report is [diarization-bench-2026-09-27-word-timings](../reports/benchmarks/diarization-bench-2026-09-27-word-timings.md).

| Split, metric | Current engine, ADR 0072 → now | Nemotron, ADR 0072 → now |
|---|---:|---:|
| AMI dev, attribution error | 22.43% → 21.07% | 14.65% → 12.49% |
| AMI dev, labelled lines with the wrong speaker | 20.57% → 21.02% | 11.03% → 11.24% |
| AMI test (held out), attribution error | 16.05% → 14.11% | 14.70% → 11.96% |
| AMI test (held out), labelled lines with the wrong speaker | 12.01% → 12.35% | 13.72% → 15.66% |
| Retained meetings, remote lines blank | 1.14% → 0.36% | 2.40% → 1.50% |

- **Where the gain comes from.** A control run scored the same transcripts with the words removed. It reached 21.07% and 12.75% on AMI dev, so the words inside segments add nothing for the current engine and 0.26 points for Nemotron. The rest comes from segment spans that follow the DTW words instead of whisper's drifting timestamps. The headline counts only reference words some line holds, and on nine AMI dev meetings the new lines hold 31,779 of them against 33,429 before, so part of the fall is words the shorter lines no longer cover. The blank remote lines are the same with and without the words.
- **`pause_ms` on AMI dev.** Attribution error, current engine and Nemotron: 100 ms 21.73% and 13.09%, 150 ms 21.59% and 12.91%, 250 ms 21.35% and 12.65%, 400 ms 21.19% and 12.51%, 500 ms 21.07% and 12.49%, 700 ms 21.07% and 12.58%, 1,000 ms 21.05% and 12.66%, 1,500 ms 21.03% and 12.68%. 500 ms is Nemotron's best and within 0.04 points of the current engine's.
- **fn-74's R2 is missed.** The spec asked for labelled lines with the wrong speaker within a point of the pre-ADR-0072 figures, 15.4% and 5.5%. They sit at 21.02% and 11.24%. A pause cut splits a two-voice line only where a gap of `pause_ms` meets a change of diarized speaker. With DTW ends most words last 20 ms, so short pauses are gaps everywhere. At 100 ms the rule splits more lines and names more of them wrongly (22.80% and 13.28%), because the diarized turns place the change a few hundred milliseconds off. At long pauses the words add nothing over the segment boundaries. Better word times cannot close this gap. Turn boundaries or the voice check (fn-70) have to.
- **Word timing accuracy.** On all of AMI dev, 61,697 Whisper words pair with the manual word times: start 170 / 430 ms, end 40 / 630 ms, pooled 120 / 490 ms (median / p95). AMI test pools 110 / 530 ms over 14,256 words. On the jfk golden large-v3-turbo pools 102 / 437 ms and tiny.en 268 / 642 ms, against Parakeet's 84 / 308 ms (`docs/reports/parakeet-alignment-report.json`, which now measures large-v3-turbo too). `scripts/qa/diarbench/word_timing.py` repeats the AMI measurement.
- **Cost per meeting hour.** Measured on a five-minute AMI excerpt, two or three runs of the engine with and without words, load included, before the timestamp mask. RTX 4090: 3.3 s without and 3.5 to 3.6 s with, about 3 s per hour. CPU: 164.4 and 153.7 s without, 157.0 and 153.2 s with, a difference inside the 10.7 s spread between runs. Integrated Radeon GPU: 242.0 and 241.6 s without, 247.5 and 276.3 s with, 1.1 to 6.9 minutes per hour, measured while a second engine shared that GPU.
- **Word error rate.** The transcript text is unchanged: the jfk fixture reads the same with and without words on tiny.en, base.en and large-v3-turbo, the Whisper CLI test holds its 0.15 ceiling, and `dettivo-qa pipeline import-merge` still merges four takes once each at a word error rate of 0.000.
- **Residual abort.** The mask covers a segment that closes early. whisper.cpp also runs DTW over the last window of the audio, and a window of 110 to 150 ms that still decodes text would abort the same way. None of the 32 bench runs hit it. The lasting fix is a guard in whisper.cpp's `whisper_exp_compute_token_level_timestamps_dtw` that skips a window of seven steps or fewer.
