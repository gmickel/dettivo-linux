# Vocabulary term recall, 2026-11-01 on thor

`dettivo-qa bench vocabulary` at `199eb1d51b84`. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips; rescoring is the time Parakeet's vocabulary rescoring added, the median over clips with a candidate span and the most on a clip without one.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER | Rescoring |
|---|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | cpu | off | 2/15 | 0 | 0.000 | - |
| whisper | large-v3-turbo | cpu | on | 10/15 | 2 | 0.000 | - |
| parakeet | parakeet-v3 | vulkan | off | 3/15 | 0 | 0.000 | - |
| parakeet | parakeet-v3 | vulkan | on | 8/15 | 0 | 0.000 | 17.4 ms median on 15 clips, 0.2 ms most without |
