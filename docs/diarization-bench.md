# The diarization bench

`just diar-bench` tells you, in under a minute, whether a change to the speaker pass puts the right speaker on more transcript lines. It scores the tree's own Rust assignment code (`crates/dettivo-meeting/src/diarize.rs`) on cached engine outputs for AMI dev and your retained English and German meetings, and prints one headline number per engine with its parts and 95% intervals. A gain on the bench is a gain in the product, because the bench runs the product's rule through `crates/dettivo-meeting/examples/diar_assign.rs` rather than a port of it.

The bench runs by hand on the desktop. It is never a CI job or a release gate ([ADR 0066](adr/0066-visual-checks-are-optional-not-a-gate.md)).

## How to hill-climb

1. Set up once with `just diar-bench-setup`, then compute the engine outputs once with `just diar-bench --full`.
2. Save where you start: `just diar-bench --save before`.
3. Change the rule in `crates/dettivo-meeting`, or add a variant (below).
4. Run `just diar-bench --baseline before`. Each metric shows its delta with a paired 95% bootstrap interval over files. A delta whose interval spans zero is noise.
5. Tune on AMI dev only. When the change is final, run `just diar-bench --heldout --baseline before` once and report the AMI test figures as they come out, without retuning on them.
6. `just diar-bench --report` writes an aggregate-only report under `docs/reports/benchmarks/` for a spec or an ADR to cite.

A change to the assignment code reruns only the assignment and scoring stages. The last line of every run lists each stage with how many results it reused (`cached`), recomputed (`computed`), or could not find (`missing`, or `stale` when it fell back to an older engine build's output). A plain run never starts an engine or Whisper. `--full` runs them wherever their inputs changed, and waits while a meeting records or finalises.

## Adding a variant

A variant is one arm of `label` in `crates/dettivo-meeting/examples/diar_assign.rs` plus its name in `VARIANTS`. It receives each recording's product `Segment`s, the engine's turns, the room-audio flag, the track length and, for Nemotron, the path of its per-10 ms speaker probabilities (`.npy`, frames by eight speakers). A two-track meeting also brings its directory, from which the product reads both tracks' levels, and one voice embedding per line. It returns the segments it labelled, and it may split, merge or drop them. Select it with `just diar-bench --variant <name>` and pass parameters with `--set <key> <value>`. The product variant takes `pause_ms`, `nearest_turn_ms` and `min_speaker_share`, the two-track rules' `bleed_min_voiced`, `single_remote`, `shared_mic`, `voiceprint` and `voice_match` ([ADR 0075](adr/0075-two-track-meetings-use-both-tracks-to-name-speakers.md)), and the voice check's `voice_check`, `voice_agree`, `voice_overrule` and `voice_evidence` (`voice`, or `probs` for an engine's frame probabilities, [ADR 0076](adr/0076-each-sentence-is-checked-against-the-speakers-voices.md)). So `just diar-bench --set min_speaker_share 0.6` tries a share floor without writing code, and `--set bleed_min_voiced 0` scores the meetings without the bleed gate. The variant and its parameters are part of every assignment cache key.

## The metrics

`scripts/qa/diarbench/metrics.py` defines every number in one table (`METRICS`). Each recording gives additive counts, a split sums them, and every metric is a ratio of sums, so a long meeting weighs more than a short one.

| Metric | What it counts |
|---|---|
| Attribution error (headline) | The share of reference words whose transcript line did not get the right speaker. A word counts against the line that holds most of it. The line is right when its speaker maps to the word's speaker under the best one-to-one map of line speakers to reference speakers, and an unlabelled line counts as wrong. Words that no line holds (speech Whisper missed) stay out of it, since no speaker choice fixes them. |
| wrong speaker, unlabelled | The headline's two parts. On a labelled set, the wrong-speaker part is the line-level word diarization error rate (WDER). |
| Short-turn error | The headline restricted to reference turns of three words or fewer, where a turn is a run of consecutive words by one speaker. |
| DER of labelled lines, and its missed, false alarm and confusion | The labelled lines as speaker turns, scored by `scripts/qa/diarization_score.py` under [ADR 0058](adr/0058-strict-diarization-accuracy-evaluation.md): zero collar, overlap included. Missed speech includes every unlabelled line. |
| Engine DER | The engine's own turns under the same scorer, before any assignment. |
| Lines right, wrong, unlabelled | fn-64's line view: a line's true speaker is the reference speaker with the most overlap, counted in lines. |
| Labelled lines, wrong speaker | The same line view over the labelled lines only: the wrong lines over the right and wrong ones. It shows what a rule that names more lines costs in wrong names. |
| Remote lines unlabelled | System-track lines of a two-track meeting left without a speaker. |
| Local/remote proxy (mix) | For meetings without labels, fn-64's engine-level proxy. Each 10 ms frame is local when the microphone is active above -45 dBFS and the system track is quiet below -60 dBFS, and remote for the reverse (`scripts/qa/nemotron3/channel_score.py`). The engine diarizes the summed tracks, each output speaker maps to the side holding most of its frames, and the figure is the share of single-side speech given to the other side. It cannot see a swap between two remote voices. |
| You lines on remote-only speech | For two-track meetings, the same proxy frames under the transcript's "You" lines: the share of their single-side frames that are remote-only. That is remote speech heard through the microphone, or words Whisper heard in silence. |
| Local speech outside You lines | The local-only frames no "You" line holds, over all local-only frames: the user's speech a gate dropped or Whisper missed. |
| Microphone lines dropped as bleed | The microphone lines the bleed gate removed, over the microphone lines the meeting stored. |
| Words the voice check fixed, broke | Reference words whose line was wrong without the voice check and right with it, and the reverse, over all words. The assignment stage labels each recording twice, with the check and without it, and each labelling gets its own best map of speakers. |
| Units the voice check moved | The units the check moved to another speaker, over the units it scored. |
| Remote lines relabelled You | Remote lines the voiceprint named You, over the remote lines. |
| Meetings under the single-remote rule, switched to the shared mic | How many two-track meetings each rule fired on. The bench holds only the system track's turns, so a meeting the product would switch reports the switch and keeps its system-track labels. |
| Engine speed | Audio seconds over the engine's wall time, model load included, from the cached run records. |
| Voice check embedding speed | Audio seconds over the wall time of the voice check's embedding call, from the cached run records. |

Intervals are 95% percentile bootstraps over files (1,000 resamples, fixed seed). A split of four files has wide intervals, and the width is the honest answer to how much a small set can tell.

## The splits

| Split | Recordings | Use |
|---|---|---|
| `ami-dev` | The 18 AMI dev meetings of the pyannote `only_words` setup, Mix-Headset audio, word timings from the AMI manual annotations | Tuning |
| `ami-test` | ES2004a, TS3003a, IS1009b and EN2002c, the four fn-64 reported on | Held out, shown only with `--heldout` |
| `local-en`, `local-de` | The retained meetings in `selection.json`, split by the meeting's language field | Proxies only, since no person labelled them |
| `labelled-en`, `labelled-de` | Meetings with a labels file (below) | The headline and the WDER on Dettivo's own audio |

Nemotron was trained on the AMI train and dev splits, so its AMI dev figures flatter it. Compare engines on AMI test and the local meetings, and use AMI dev to tune the rule for a fixed engine.

AMI has no product transcript. Whisper large-v3-turbo through the product's engine CLI cuts each recording into segments with timed words ([ADR 0074](adr/0074-whisper-times-every-word-by-dtw-for-speaker-labelling.md)), and zero-length segments are dropped. Meetings use the segments the product stored, read-only from the database. To measure a Whisper change on your own meetings before any stored meeting carries it, set `"asr": {"meetings": true}` in `bench.json`. The bench then runs each meeting's takes through the product's finalisation with the configured Whisper engine (`crates/dettivo-meeting/examples/meeting_asr.rs`), caches the result like the AMI transcripts, and `--full` computes it.

## Voice embeddings

The voiceprint rule reads one embedding per line of at least 1.5 seconds, each on its own track. The bench builds the tree's `dettivo-engine-diarize`, runs its `--embed` mode with the product's diarization model over each two-track meeting, and caches the vectors under `cache/embed/`, keyed by the engine binary, the model, both tracks and the spans. `--full` computes the missing ones. A plain run without them scores the meeting as if the engine had no `embed` request.

The voice check ([ADR 0076](adr/0076-each-sentence-is-checked-against-the-speakers-voices.md)) embeds each sentence unit on the diarized track, and the units depend on the rule. The bench first asks the assignment stage for every job's unit spans (`mode: spans`), then embeds them with the tree's engine on the recording's diarized track, caches the vectors under `cache/embed/` keyed by the engine binary, the model, the track and the spans, and sends them back with the job (`unit_embeddings`). `--full` computes the missing ones, and a plain run without them scores the check as if no voice came back. `--set voice_check false` needs no embeddings, and neither does `--set voice_evidence probs`.

## Word timing

`python3 scripts/qa/diarbench/word_timing.py` tells you how close the cached Whisper word times sit to the AMI manual word times on AMI dev (`--heldout` measures AMI test instead). It pairs a Whisper word with a reference word of the same text inside runs of at least three matching words, so a lone "the" never pairs across a minute, and prints the median and p95 of the start, end and pooled offsets with the paired count. It reads the ASR cache of the configured engine, so run `just diar-bench --full` first. The output holds pooled figures only.

## Labels

A labels file joins the bench automatically when it exists at `<eval>/labels/<alias>.json`. `just diar-label <alias>` writes it from a keyboard-driven page on 127.0.0.1, and [Labelling meetings](diarization-labelling.md) covers the page and the rules for who said a line:

```json
{"schema": 1, "alias": "DE-2", "language": "de", "draft": "blend:current+nemotron",
 "window_ms": [600000, 1500000],
 "lines": [{"start_ms": 601200, "end_ms": 604900, "source": "system", "speaker": "B", "words": 11}]}
```

`speaker` is any stable name or letter, and null marks a line the labeller could not attribute, which the bench skips. `source` is `system` or `microphone`, and a unit only matches lines of its own source. `window_ms` is optional and limits the meeting's lines to a labelled excerpt, and the split's minutes then count the excerpt. `draft` records which systems seeded the page, and the bench does not read it.

## Where things live

Everything the bench reads or writes, other than the retained meetings and the database it only reads, sits in the protected eval directory, `~/.local/share/dettivo-eval/bench` (mode 700, or `$DETTIVO_EVAL_DIR`). That includes AMI audio and references under `ami/`, `selection.json` (aliases to meeting directories), `labels/`, `cache/`, one scoreboard per run under `runs/`, and `baselines/`. `bench.json` there can override an engine (`binary`, `model`, `threads`, `provider`, and `probs` to keep a diarize engine's frame probabilities) or add one. The engines default to the installed product binaries under `/usr/lib/dettivo/engines/`: `current` (sherpa-onnx) and `nemotron-cpp` (`dettivo-engine-nemotron` on the CPU with the catalogue's `nemotron-3-diarization`, whose probabilities go to the cache as the runner's do), plus the fn-64 Nemotron ONNX runner (`scripts/qa/nemotron3/nemotron3_diarize.py`, int8 on the CPU) as `nemotron`. A run skips an engine whose binary is not installed and says so, so a tree built before the Nemotron engine was installed still runs. To bench a build, point `nemotron-cpp` at it in `bench.json`, or add a CUDA row with `"provider": "cuda"`.

Nothing confidential enters git. The scoreboards hold per-file counts keyed by alias, and they stay in the eval directory. `--report` writes pooled numbers, file counts, audio minutes and engine hashes only, and `scripts/qa/test_diarbench.py` holds the writer to that.

## Setup

`just diar-bench-setup` runs these steps, each skipped when already done:

1. It creates the eval directory with mode 700.
2. It creates a Python environment with numpy 2.5.3 and onnxruntime-gpu 1.30.0, through `uv` when present.
3. It fetches the AMI dev and test audio, the pyannote references (checked against a pinned manifest hash) and the AMI manual annotations (checked against a pinned hash).
4. It fetches the Nemotron int8 model and checks it against the fn-64 receipt. It also checks that the product's diarization model and Whisper large-v3-turbo are installed.
5. It writes `selection.json` from the finished retained meetings in English and German with one take per track of equal length, at least two minutes long. It skips any meeting that is recording, finalising, or waiting on its speaker pass or analysis, and it keeps existing aliases.
6. When fn-64's working directory (`~/.local/share/dettivo-eval/fn64`) exists, it seeds the cache with that evaluation's engine outputs, probabilities and Whisper segments, provided the binaries and models still hash to what the fn-64 receipt recorded.

## Results

[ADR 0076](adr/0076-each-sentence-is-checked-against-the-speakers-voices.md) added the voice check, measured in the [voice-check report](reports/benchmarks/diarization-bench-2026-09-28-voice-check.md). AMI dev attribution error fell from 21.07% to 18.61% with the current engine and from 12.49% to 11.98% with Nemotron, and the held-out AMI test from 14.11% to 11.81% and from 11.96% to 11.54%. The check fixed 3.89% of AMI dev words and broke 1.41% with the current engine, and fixed 1.57% and broke 1.06% with Nemotron.

[ADR 0075](adr/0075-two-track-meetings-use-both-tracks-to-name-speakers.md) added the two-track rules, measured in the [two-track report](reports/benchmarks/diarization-bench-2026-09-27-two-track.md). The bleed gate cut the share of "You" line time on remote-only speech from 37.86% to 22.00% on the retained meetings, at a cost of 0.32 points of local speech. The single-remote rule fired on 2 of the 10 meetings. The voiceprint relabelled no remote line, and no meeting switched to the shared microphone. AMI is room audio, and none of its figures moved.

[ADR 0074](adr/0074-whisper-times-every-word-by-dtw-for-speaker-labelling.md) gave Whisper word timings and retuned `pause_ms` to 500, measured in the [word-timings report](reports/benchmarks/diarization-bench-2026-09-27-word-timings.md). AMI dev attribution error fell from 22.43% to 21.07% with the current engine and from 14.65% to 12.49% with Nemotron, and blank remote lines on the retained meetings fell to 0.36% and 1.50%. Labelled lines with the wrong speaker stayed at 21.02% and 11.24%.

[ADR 0072](adr/0072-every-remote-line-takes-the-speaker-of-its-sentence.md) replaced the coverage and share rule with the sentence rule, measured in the [sentence-units report](reports/benchmarks/diarization-bench-2026-09-26-sentence-units.md). It cut AMI dev attribution error from 28.99% to 22.43% with the current engine and from 22.43% to 14.65% with Nemotron. Remote lines left blank on the retained meetings fell from 18.0% to 1.1% and 2.4%.

## First results

The [report of 2026-09-26](reports/benchmarks/diarization-bench-2026-09-26.md) is the starting point for fn-68 to fn-72. Today's rule leaves 15.1% of AMI dev words on unlabelled lines with either engine, and gives 13.9% of them a wrong speaker with the current engine against 7.6% with Nemotron.

On the inputs fn-64 used, the bench reproduces [that report's](reports/benchmarks/diarization-nemotron3-2026-09-25.md) current-engine figures exactly. On AMI test that is 15.67% engine DER with 3.39% confusion, and on Whisper lines 1.97% confusion, 25.60% missed and 6.75% false alarm, with 75.6% of lines right, 5.5% wrong and 18.9% unlabelled. On the eight retained meetings it is 22.7% of remote lines unlabelled and 0.88% local/remote confusion on the mix. The local splits now hold ten meetings, so their pooled figures differ from fn-64's.
