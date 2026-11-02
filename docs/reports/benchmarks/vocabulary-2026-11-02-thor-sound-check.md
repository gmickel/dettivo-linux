# Vocabulary term recall, 2026-11-02 on thor

`dettivo-qa bench vocabulary` at `fea561bc01ed`. Each clip's text has passed dictation's output guards. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips; time added is what Parakeet's rescoring or Whisper's term check added, the median over clips with a candidate span and the most on a clip without one.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Time added |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | vulkan | off | 2/15 | 0 | 0.011 | - |
| whisper | large-v3-turbo | vulkan | on | 10/15 | 0 | 0.000 | 0.0 ms median on 13 clips, 0.0 ms most without |
| parakeet | parakeet-v3 | vulkan | off | 3/15 | 0 | 0.000 | - |
| parakeet | parakeet-v3 | vulkan | on | 13/15 | 0 | 0.000 | 13.8 ms median on 26 clips, 1.4 ms most without |

## Single names

One word said on its own, with the same vocabulary. Term recall counts listed names that came back; false insertions count lone sound-alikes that became a term. The set has no guard clips.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Time added |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | vulkan | on | 6/10 | 0 | 0.000 | 34.6 ms median on 8 clips, 0.0 ms most without |
