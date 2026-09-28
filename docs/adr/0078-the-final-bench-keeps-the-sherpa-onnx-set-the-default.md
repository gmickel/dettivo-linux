# 0078. The final bench keeps the sherpa-onnx set the default

Status: Accepted 2026-09-28, supersedes the default-engine decision of [0073](0073-nemotron-3-diarization-runs-through-nemo-speech-cpp.md)

## What this gives you

A meeting's speakers still come from the sherpa-onnx set (`diarization-en`) unless you choose Nemotron 3 Diarization. With every speaker-labelling rule of 0.4.0 in place, the two engines tie on held-out AMI meetings and on German meetings, and the sherpa-onnx set labels English calls with several remote voices better. Nemotron remains one setting away, `[meetings.diarization] model = "nemotron-3-diarization"`, when speed matters more: it diarizes at about 330x realtime against about 15x.

## Situation

[0073](0073-nemotron-3-diarization-runs-through-nemo-speech-cpp.md) shipped Nemotron opt-in and deferred the default to a bench run after the labelling rules landed. Its rule makes Nemotron the default only if it is ahead of the current engine on the pooled headline number and not behind on German, both within the bench's intervals. Since then the speaker pass gained the sentence rule ([0072](0072-every-remote-line-takes-the-speaker-of-its-sentence.md)), Whisper word timings ([0074](0074-whisper-times-every-word-by-dtw-for-speaker-labelling.md)), the two-track rules ([0075](0075-two-track-meetings-use-both-tracks-to-name-speakers.md)) and the voice check ([0076](0076-each-sentence-is-checked-against-the-speakers-voices.md)). No German meeting carries hand labels, so German has no headline number.

## Decision

The fn-67 bench ran both engines on 2026-09-28 with every product default on, over 18 AMI dev meetings, the 4 held-out AMI test meetings and ten local meetings, six of them German ([report](../reports/benchmarks/diarization-bench-2026-09-28.md)). Paired 95% intervals, Nemotron minus the current engine:

| Split | Metric | Delta, points | 95% interval |
|---|---|---:|---:|
| AMI dev, 18 meetings | Attribution error (headline) | −6.60 | [−11.68, −2.20] |
| AMI test, 4 meetings, held out | Attribution error (headline) | −0.30 | [−0.71, +1.11] |
| AMI test, 4 meetings, held out | Labelled lines, wrong speaker | +3.28 | [+0.63, +5.89] |
| German meetings, 6 | Remote lines unlabelled | +0.25 | [+0.12, +0.35] |
| German meetings, 6 | Local/remote proxy (mix) | −0.76 | [−1.58, −0.40] |
| English meetings, 4 | Remote lines unlabelled | +1.76 | [+0.74, +3.09] |

Nemotron is ahead only on AMI dev, a split it was trained on. On the held-out meetings it ties on words and gets more labelled lines wrong.

To judge the local meetings without labels, a model judge read them blind, by fn-64's method. Each engine's labels sat in one of two shuffled columns with speakers renumbered, and the judge named the column that fit who was addressed, the turn-taking and the self-references. On the remote rows the two engines labelled differently, the judge sided with each engine 13 times on German meetings. On English meetings it sided with the current engine 29 times and with Nemotron 10 times (sign test p = 0.003). Almost all of that gap is one call where Nemotron merged four remote voices into two. In random windows of 60 rows both engines put about 0.6% of remote rows under the wrong speaker, which is the judge's resolution.

The rule is not met, so `[meetings.diarization] model` stays `diarization-en`, and `Diarization::default` and the default configuration text are unchanged.

## Consequences

- Nothing changes for an existing configuration: a file that names either model keeps it.
- Nemotron's two known gaps decide its next attempt at the default. It misses about twice the speech of the current engine (20.1% engine missed speech on AMI test against 9.3%), which leaves more remote lines without a speaker. It also undercounts speakers when a call has several remote voices. A later bench that meets the rule after those are fixed, or a hand-labelled German meeting that shows the engines apart, reopens the decision.
- The judge's packets and results hold transcript text, so they stay in the evaluation directory on the machine that ran them. The repository keeps only the aggregate report.
