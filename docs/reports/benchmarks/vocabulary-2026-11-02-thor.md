# Vocabulary term recall, 2026-11-02 on thor

`dettivo-qa bench vocabulary` at `60efadb890e7`. Each clip's text has passed dictation's output guards. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips; time added is what Parakeet's rescoring or Whisper's term check added, the median over clips with a candidate span and the most on a clip without one.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Time added |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | vulkan | off | 2/15 | 0 | 0.011 | - |
| whisper | large-v3-turbo | vulkan | on | 10/15 | 0 | 0.000 | 0.0 ms median on 13 clips, 0.0 ms most without |
| parakeet | parakeet-v3 |  | off | skipped: dettivo-engine-parakeet is not built | - | - | - |
| parakeet | parakeet-v3 |  | on | skipped: dettivo-engine-parakeet is not built | - | - | - |
