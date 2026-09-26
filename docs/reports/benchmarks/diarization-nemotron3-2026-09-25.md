# Nemotron 3 Diarization against the current speaker pass

This report tells a follow-up spec what swapping or adding NVIDIA Nemotron 3 Diarization would buy on this machine. Nemotron assigns far fewer words to the wrong speaker on labelled public meetings and runs about four times faster on the CPU. It also misses about twice as much speech. On Dettivo's own retained meetings, most of which have one to four remote voices, the two systems agree on almost every transcript line. Today's speaker pass leaves 23% of remote transcript lines without a speaker, and Nemotron under the same rule leaves 24%. A segment-based adapter, which gives each line the Nemotron speaker with the highest mean probability over the line, labels every line, and the blind judge found no wrong speaker in any of the 840 lines it labelled. On AMI, the same adapter gets 86% of Whisper lines right against 76% for the current engine, and it also gives 13.8% a wrong speaker against 5.5%. The shipping default stays the current `dettivo-engine-diarize` (R4). Nothing in the daemon, the packages, `config.toml` or Settings changed.

| AMI test split, 4 meetings, 126 minutes | Current engine, CPU | Current engine, CUDA | Nemotron int8, CPU | Nemotron fp32, CUDA |
|---|---:|---:|---:|---:|
| Wrong-speaker rate (DER confusion) | 3.39% | 3.40% | 0.71% | 0.73% |
| Missed speech | 9.25% | 9.25% | 20.16% | 20.07% |
| False alarm | 3.03% | 3.03% | 0.91% | 0.88% |
| Total DER | 15.67% | 15.67% | 21.78% | 21.68% |
| Exact speaker counts | 4/4 | 4/4 | 3/4 | 3/4 |
| Wall time, cold process | 557 s | 510 s | 138 s | 14.4 s |
| Realtime factor | 13.6x | 14.9x | 54.8x | 524.8x |
| Engine process CPU time | 2,208 s | 508 s | 1,004 s | 266 s |
| Peak process RSS | 665 MiB | 1,335 MiB | 785 MiB | 1,463 MiB |

On the CPU, Nemotron's wall time is 25% of the current engine's. That matches Meeting Recorder 1.1's claim of about a quarter of the time. Its wrong-speaker rate is one fifth of the current engine's. Its extra missed speech makes total DER worse by 6.1 points, which matters for a transcript that labels every segment.

## What was compared

- **Current engine (baseline).** The installed `dettivo-bin` 0.2.0-1 `dettivo-engine-diarize`, which is the shipping default. It uses the `diarize/diarization-en` model set, runs in automatic count mode with four threads, and uses the CPU provider. The CUDA row is the same crate source, unchanged since 0.2.0, built with `--features cuda` and run with `--provider cuda`.
- **Nemotron 3 Diarization.** `scripts/qa/nemotron3/nemotron3_diarize.py` ports omarchy-meeting-recorder v1.1.0's ONNX inference and post-processing to Python on ONNX Runtime 1.30.0. The recorder's post-processing thresholds speaker probability at 0.5, bridges same-speaker pauses under 500 ms, drops runs under 300 ms and absorbs small clusters. The CPU row uses the recorder's int8 graph and the CUDA row uses the fp32 graph, both from `onnx-community/Nemotron-3-Diarization-ONNX` at revision `353b6f8a`. The model licence is OpenMDW 1.1.
- **Nemotron, segment-based.** `scripts/qa/nemotron3/segment_assign.py` is a Nemotron-specific adapter over the int8 port's per-10 ms probabilities. Each transcript segment takes the speaker with the highest mean probability over its span, with no 0.5 threshold. Only the channels the recorder post-processing keeps as speakers compete, so the adapter adds no speaker that the recorder's turns lack. A segment goes without a speaker only when it has no audio frames.
- **Port fidelity.** Over the first 600 s of AMI ES2004a, which spans 22 chunks and exercises the speaker-cache compression, the fp32 port's per-frame probabilities match the transformers reference implementation of `nvidia/Nemotron-3-Diarization` (revision `f667ed7`, float32, CPU) within 1.1e-5. No speech decision differs and the turns are identical. The int8 graph differs from that reference in 0.04% of speech decisions, with 0.50% turn-level DER and no confusion.

The [machine-readable receipt](diarization-nemotron3-2026-09-25.json) holds every score, the binary, library and model hashes, the host and the cross-check.

## Labelled public meetings

`scripts/qa/diarization_score.py` does the scoring under the [ADR 0058](../../adr/0058-strict-diarization-accuracy-evaluation.md) protocol. It uses zero collar, includes overlapping speech, takes the UEM union and maps speakers one to one. The references are the pyannote AMI-diarization-setup `only_words` RTTM and UEM files, and pooled figures weight each meeting by its reference speaker-seconds. The wrong-speaker rate is DER's confusion component, which is the share of reference speech that goes to the wrong speaker after the best speaker mapping.

Nemotron was trained on the AMI train and dev splits and on VoxConverse, so the headline table uses only the AMI test split: ES2004a, TS3003a, IS1009b and EN2002c. The earlier reports' train-split meetings, ES2002a and EN2001b, appear below only for continuity. They favour Nemotron and are not a fair comparison.

| Per-meeting wrong-speaker rate, automatic count | ES2004a | TS3003a | IS1009b | EN2002c |
|---|---:|---:|---:|---:|
| Current engine, CPU | 4.02% | 2.19% | 3.42% | 3.57% |
| Nemotron int8, CPU | 0.31% | 2.41% | 0.83% | 0.23% |

Nemotron's one weak meeting, TS3003a, is where it found two speakers instead of four. EN2002c has three annotated speakers, and both systems found three.

**Known four-speaker control.** The control passes `--speakers 4` on all four meetings, as the earlier reports did, so it overstates EN2002c's count by one and neither system can score an exact count there. With it, Nemotron's wrong-speaker rate falls to 0.43% and TS3003a drops to 0.34%. The current engine rises to 7.87% on the CPU, because IS1009b jumps to 17.96% confusion in CPU known-count mode, against 3.21% on CUDA. The CUDA engine scores 4.10%. The CPU and CUDA builds therefore diverge in known-count mode on that meeting, and nothing here isolates why. The automatic mode the product uses matched across providers within 0.01 points.

**Train-split continuity.** On ES2002a and EN2001b, the current engine scores 11.65% DER with 1.99% confusion and two exact counts. Nemotron scores 19.09% DER with 1.71% confusion, returning three speakers for four on both meetings.

**Missed speech is the model, and post-processing does not cause it.** Running the port with `--postprocess none`, which keeps the raw over-0.5 runs, raised missed speech on every test meeting, for example from 21.37% to 25.83% on ES2004a. The recorder's bridging recovers speech and does not lose it. Nemotron's model card reports lower DER on AMI test than seen here, but it scores against forced-alignment references. This report scores against the `only_words` references, so the two sets of figures do not compare directly.

## Dettivo's own meetings

The evaluation covered eight of the eleven finished retained meetings, all with both tracks, 416 minutes per track. Four are English (277 minutes) and four are German (139 minutes, three recorded on 2026-09-25). The meeting's `language` field decided the class. The meeting that was recording during the evaluation, one failed and one finished clip under two minutes, and two German meetings left out to bound run time were not used. No person has labelled these meetings, so they have no true wrong-speaker reference. Three proxies stand in for one, and none of them is a human-labelled rate.

| Proxy | Class | Current engine | Nemotron |
|---|---|---:|---:|
| Local/remote confusion on the mix | English | 0.63% | 0.57% |
| | German | 1.37% | 0.21% |
| | Pooled | 0.88% | 0.45% |
| Single-side speech missed on the mix | Pooled | 0.63% | 1.25% |
| Overlap frames with both sides active in the output | English | 72.1% | 77.4% |
| | German | 77.2% | 79.0% |
| | Pooled | 75.0% | 78.3% |
| Judged wrong-speaker lines, system track | English | 2 of 357 (0.56%) | 4 of 363 (1.10%) |
| | German | 0 of 363 (0%) | 0 of 343 (0%) |
| | Pooled | 2 of 720 (0.28%) | 4 of 706 (0.57%) |
| Remote lines left without a speaker in the judged windows | English | 95 | 89 |
| | German | 5 | 25 |

The mix rows come from the CPU runs. The CUDA runs of each system score within 0.15 points of them.

**Track weak reference (mix).** The microphone holds the local speaker and the system track holds the remote participants. `scripts/qa/nemotron3/channel_score.py` marks each 10 ms frame `local` when the microphone is active above -45 dBFS and the system track is quiet below -60 dBFS, `remote` for the reverse, and `overlap` when both are active. The 15 dB gap between the two levels is the bleed margin. Each system diarized the summed mix, and each output speaker maps to the side holding most of its frames. Local/remote confusion is the share of scored speech given to a speaker from the other side. It is a lower bound on the wrong-speaker rate, because it cannot see a swap between two remote voices. Nemotron halves it pooled and cuts it from 1.37% to 0.21% on German. Overlap made up 5.9% of scored speech, 3.9% in English and 9.6% in German. Nemotron put both sides on 3.3 points more overlap frames, a modest crosstalk gain.

**Judged transcripts (system track).** The system track is the product's actual diarization input. `scripts/qa/nemotron3/judge_pack.py` took the product's own timed Whisper segments for each meeting and gave each remote segment the speaker of each system's turns with the most overlap. Three windows of 60 segments per meeting were sampled with a fixed seed, and the two systems were shown as columns A and B in random order with their labels renumbered. A model judge, one per language class, read each window without the A/B key. It counted remote lines whose label was plausibly wrong from turn-taking, addressing and sentence continuity, and it logged doubtful lines separately. Across the 24 windows the two systems gave the same speaker to all but 5 of the 688 remote lines both labelled. The judged rates are therefore too close to separate the systems. Nemotron left more German lines without a speaker (25 against 5), consistent with its higher missed speech.

**Cross-system agreement.** On the system track, Nemotron's turns differ from the current engine's by 10.9% DER pooled, and 1.1% of that is speaker confusion. The remote-speaker counts matched on seven of eight meetings. On one English meeting the current engine found four remote speakers and Nemotron found two. The judged windows did not show which count is right.

## Speakers on transcript lines

This section scores what a user sees, which is a speaker on each transcript line. The speaker pass in `crates/dettivo-meeting/src/diarize.rs` gives a line the most-overlapping speaker when at least 25% of the line lies inside diarized speech and that speaker holds at least 60% of it (`[meetings.diarization]` `min_coverage` and `min_speaker_share`). Otherwise the line gets no speaker. The current engine and Nemotron with the recorder post-processing go through that rule. The segment-based adapter replaces it with the mean-probability choice described above. `scripts/qa/nemotron3/score_segments.py` scores all three.

**Rule port check.** The Python port of the rule, run on the current engine's CPU turns, reproduces the labels the product stored for the same eight meetings. It makes the same labelled-or-not decision on 4,534 of 4,545 remote lines, and it picks the same speaker on 3,509 of the 3,510 lines both labelled.

| Dettivo meetings, remote lines | Current engine | Nemotron, recorder post-processing | Nemotron, segment-based |
|---|---:|---:|---:|
| Left without a speaker, English (2,899 lines) | 765 (26.4%) | 784 (27.0%) | 0 |
| Left without a speaker, German (1,646 lines) | 266 (16.2%) | 329 (20.0%) | 0 |
| Left without a speaker, pooled (4,545 lines) | 1,031 (22.7%) | 1,113 (24.5%) | 0 |
| Judged wrong-speaker lines, English | 0 of 396 | 0 of 396 | 0 of 475 |
| Judged wrong-speaker lines, German | 0 of 303 | 0 of 292 | 0 of 365 |
| Judged wrong-speaker lines, pooled | 0 of 699 | 0 of 688 | 0 of 840 |

**Fresh blind pack.** `judge_pack.py --rule product` drew new windows from the same eight meetings with seed 65, three windows of 60 lines per meeting, and showed the three systems as columns A, B and C in random order per window. The counting protocol and the one-judge-per-language setup match the first pack. Both judges' counts of labelled and unlabelled lines matched the packets in all 72 columns, so each judge read every line. Neither judge found a wrong speaker in any column. The adapter labelled the 141 to 152 window lines the rule left blank, and the judges found none of those wrong either. Zero in 840 puts the adapter's judged rate below about 0.4% at 95% confidence, which is also the judge's resolution on these meetings.

| AMI test split, 4 meetings, automatic count | Current engine | Nemotron, recorder post-processing | Nemotron, segment-based |
|---|---:|---:|---:|
| **Reference-word segments (1,526)** | | | |
| Wrong-speaker rate (DER confusion) | 1.06% | 0.61% | 1.20% |
| Missed speech | 16.72% | 16.97% | 5.70% |
| Lines left without a speaker | 37.2% | 38.1% | 0% |
| Lines with the right speaker | 53.7% | 51.3% | 71.9% |
| Lines with a wrong speaker | 9.1% | 10.6% | 28.1% |
| **Whisper segments (3,026, of which 2,820 hold reference speech)** | | | |
| Wrong-speaker rate (DER confusion) | 1.97% | 1.78% | 3.39% |
| Missed speech | 25.60% | 25.95% | 16.65% |
| False alarm | 6.75% | 6.26% | 16.50% |
| Lines left without a speaker | 18.9% | 19.7% | 0% |
| Lines with the right speaker | 75.6% | 73.6% | 86.2% |
| Lines with a wrong speaker | 5.5% | 6.8% | 13.8% |

**How AMI is segmented.** AMI has no product transcript, so the scorer cuts each meeting into segments twice. The first cut uses the `only_words` reference turns, where each segment is one speaker's run of words. The second uses the installed `dettivo-engine-whisper` with large-v3-turbo on Vulkan over each whole recording, which took 74 s for the four meetings. Every system labels the same segments. The labelled segments become turns that `diarization_score.py` scores under ADR 0058, so missed speech here includes every line left without a speaker, plus the second voice wherever speech overlaps. The line rows give each line the reference speaker with the most overlap, and count a labelled line wrong when its label differs under the best one-to-one mapping of labels to reference speakers. Every reference-word segment lies inside reference speech, so false alarm there is 0% for every system and the table omits that row.

**Where the adapter's extra errors come from.** On the lines the rule also labels, the adapter matches the recorder's confusion, 0.63% against 0.61% on reference-word segments and 1.81% against 1.78% on Whisper segments. All of its extra confusion therefore comes from the lines the rule refuses, which on AMI are mostly short backchannels and overlapped speech. Against the current engine, the adapter gains 18.2 points of right-speaker lines on reference-word segments and 10.6 points on Whisper segments, and it adds 19.0 and 8.3 points of wrong-speaker lines. On Whisper segments it also adds about 10 points of false alarm, because it labels line spans that the reference marks as silence. Nemotron's two-speaker count on TS3003a, against four in the reference, lifts that meeting's adapter confusion to 4.62% and 6.52%.

## What the numbers support

- **Wrong-speaker rate.** Nemotron is better on labelled public meetings: 0.71% against 3.39% confusion on the AMI test split, and 0.43% against 4.10% (CUDA) with the count known. On Dettivo meetings the only measurable difference is the local/remote proxy, 0.45% against 0.88% pooled. The line-level judge finds both systems near zero, and in the fresh three-column pack it found no wrong line for any system. These meetings have too few remote voices to reproduce Meeting Recorder's 17% and 54% baseline rates, and the two systems' remote-speaker counts agreed on seven of eight.
- **Missed speech.** Nemotron misses 20.2% of reference speech on the AMI test split against 9.3%, and twice the single-side speech on Dettivo meetings. An integration that takes Nemotron's speaker labels while keeping the current engine's speech regions would sidestep this, and a follow-up spec could evaluate that combination.
- **Segment-based assignment.** On Dettivo meetings the adapter closes the gap users see most. The speaker pass leaves 22.7% of remote lines blank with the current engine, the adapter leaves none, and the judge found no wrong speaker among the extra lines. On AMI, where four people share one room and talk over each other, the adapter gets 10.6 to 18.2 points more lines right than the current engine and 8.3 to 19.0 points more wrong, because every line it adds is one the rule refused as ambiguous. A follow-up spec could run the adapter only on the lines the rule leaves blank, which keeps the rule's accuracy on the lines it already labels. On those lines the adapter's confusion already matches the recorder's, 0.63% against 0.61%.
- **Time.** On the CPU with four threads, Nemotron takes a quarter of the current engine's wall time and 45% of its CPU time. On CUDA it runs at about 525 to 775 times realtime against 15 to 19 for the current engine. The current engine's CUDA build saves CPU time (508 s against 2,208 s) but only 9% of wall time on the test split.
- **Languages.** The German meetings show the largest proxy gain, 1.37% to 0.21% local/remote confusion. No labelled German corpus was cheap to get. VoxConverse is part of Nemotron's training data, and CALLHOME German is not freely licensed. The German figures therefore rest on the proxies alone.

## Limits and open items

- **No human labels.** No person labelled the retained meetings, so this report gives no true wrong-speaker rate for them. The open item for a follow-up spec is labelling. One person annotating speaker turns on two or three multi-speaker meetings, at least one of them German, would turn the proxies into a rate.
- **The judge is a model.** It reads text only, it cannot hear voices, and it saw 24 windows with 688 doubly labelled remote lines. A 0.3-point difference is below what it can resolve. In the three-column pack, the adapter's column never shows an unlabelled line, so a judge could tell it apart from the other two without the key. Both packs were drawn before a sampling fix: the window picker never offered a meeting's last full window when its segment count was an exact multiple of the window size, so a rebuild with the same seed now draws different windows. The scoring is unaffected.
- **AMI segments are a stand-in.** AMI has no product transcript. The reference-word cut is an oracle segmentation, and the Whisper cut ran the engine CLI over each whole recording, where the product transcribes in its own windows. Both cuts give the three systems identical segments, so the comparison between them holds, and the absolute line rates would move with a different cut.
- **Timing conditions.** CPU jobs ran at `nice 19` with idle I/O priority while a meeting was being recorded on this machine. The AMI CPU jobs ran one at a time. The second batch of real-meeting CPU jobs overlapped the first, so real-meeting CPU times are indicative only and the table above uses AMI times. CUDA jobs waited until no meeting was recording or finalising. Every wall time is a cold process with model load included, and nothing pinned CPU affinity.
- **Runtime.** The Nemotron port runs in Python with NumPy features, while the recorder runs in Rust. A native integration would change feature-extraction time. It would not change the ONNX Runtime inference, which dominates the CPU time.

## Reproduce

Working files, audio links, outputs, judge packets and results stay in a protected directory outside the repository and never enter git. The scripts take paths.

```sh
python3 scripts/qa/nemotron3/prepare_meetings.py --selection selection.json --out real
python3 scripts/qa/nemotron3/run_eval.py --jobs jobs.json --systems systems.json --out runs
python3 scripts/qa/nemotron3/judge_pack.py --selection selection.json --runs runs \
  --systems baseline-cpu,nemotron-cpu-int8 --out judge
python3 scripts/qa/nemotron3/score_eval.py --work . \
  --systems baseline-cpu,baseline-cuda,nemotron-cpu-int8,nemotron-cuda-fp32
python3 scripts/qa/nemotron3/nemotron3_diarize.py --wav real/EN-1/system.wav \
  --model model_quantized.onnx --probs probs/EN-1.system.npy
python3 scripts/qa/nemotron3/judge_pack.py --selection selection.json --runs runs --rule product \
  --probs probs --systems baseline-cpu,nemotron-cpu-int8,nemotron-cpu-int8+segment --seed 65 --out judge-r6
python3 scripts/qa/nemotron3/score_segments.py --work . --selection selection.json --probs probs \
  --whisper whisper --systems baseline-cpu,nemotron-cpu-int8,nemotron-cpu-int8+segment
python3 scripts/qa/nemotron3/crosscheck_reference.py --wav ES2004a.wav \
  --hf-model <nvidia/Nemotron-3-Diarization> --onnx model.onnx
```

The Nemotron scripts need `numpy` and `onnxruntime-gpu`. The cross-check additionally needs `torch`, `librosa` and transformers with `nemotron3_diarization` support. `run_eval.py` waits with GPU jobs while any Dettivo meeting records or finalises, and it only reads the Dettivo database. `--probs` runs once per AMI test recording (`probs/<name>.npy`) and once per real system track, and `whisper/<name>.json` is `dettivo-engine-whisper --json` on each AMI test recording.
