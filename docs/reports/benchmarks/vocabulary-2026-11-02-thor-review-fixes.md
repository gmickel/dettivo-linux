# Vocabulary term recall, 2026-11-02 on thor

`dettivo-qa bench vocabulary` at `7412399d17ea`. Each clip's text has passed dictation's output guards and its default raw layer, so it is the text raw dictation pastes. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips; time added is what Parakeet's rescoring or Whisper's term check added, the median over clips with a candidate span and the most on a clip without one.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Time added |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | vulkan | off | 2/15 | 0 | 0.021 | - |
| whisper | large-v3-turbo | vulkan | on | 10/15 | 0 | 0.011 | 0.0 ms median on 13 clips, 0.0 ms most without |
| parakeet | parakeet-v3 | vulkan | off | 3/15 | 0 | 0.011 | - |
| parakeet | parakeet-v3 | vulkan | on | 13/15 | 0 | 0.011 | 18.1 ms median on 26 clips, 1.9 ms most without |

## Single names

One word said on its own, with the same vocabulary. Term recall counts listed names that came back; false insertions count lone sound-alikes that became a term. The set has no guard clips.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Time added |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | vulkan | on | 6/10 | 0 | 0.000 | 47.8 ms median on 8 clips, 0.0 ms most without |

## Against the sound-check report

The vocabulary check now reads words, marks and anchors as the PR #49 review asked (ADR 0095), and each clip also passes the default raw layer (ADR 0096). 204 of the 208 transcripts are identical to [vocabulary-2026-11-02-thor-sound-check](vocabulary-2026-11-02-thor-sound-check.md). The other four are guard-library on every row: raw dictation applies the spoken "period" after "exam" as a full stop, so "during the exam period." pastes as "during the exam.". That one deletion raises every row's guard WER by 0.011 and changes no term figure.
