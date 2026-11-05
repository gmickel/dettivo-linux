# Vocabulary term recall, 2026-11-01 on thor

`dettivo-qa bench vocabulary` at `9d0843136b77`. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips.

| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER |
|---|---|---|---|---|---|---|
| whisper | large-v3-turbo | cpu | off | 2/15 | 0 | 0.000 |
| whisper | large-v3-turbo | cpu | on | 10/15 | 2 | 0.000 |
| parakeet | parakeet-v3 | cpu | off | 3/15 | 0 | 0.000 |
| parakeet | parakeet-v3 |  | on | skipped: dettivo-engine-parakeet takes no vocabulary | - | - |
