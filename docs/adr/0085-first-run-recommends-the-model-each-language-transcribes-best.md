# 0085. First run recommends the speech model each language transcribes best, from a recorded FLEURS run

Status: Accepted 2026-11-01. Amends 0024 (which model the Models step marks and preselects).

## What this gives you

A German speaker starting Dettivo for the first time sees the model that transcribes German best preselected, with its word error rate beside it, and `dettivo speech providers --language de` lists every scored model in order. An install that already has a model keeps it.

## Situation

First run offered the catalogue's shortlist and marked the default for the GPU tier, whatever the person speaks. The models differ by language. Parakeet v2 is English-only, Parakeet v3 and Parakeet Ultra cover 25 European languages, and Whisper large-v3-turbo covers 99.

The `language_wer` step of `dettivo-qa bench` scored every catalogue speech model on disk on the first 100 utterances of the FLEURS test split in eight languages (`scripts/qa/fetch-fleurs.sh`), on the reference machine's Vulkan engines. The rate is the corpus word error rate against FLEURS's normalised transcription, with the engine's text lower-cased and split on anything that is not a letter or digit. Digits against spelled numbers count as errors for every model alike. Rates in percent from [the recorded run](../reports/benchmarks/2026-11-01-thor-gpu.json):

| Language | Parakeet Ultra | Parakeet v3 | Parakeet v2 | Whisper large-v3-turbo |
|---|---|---|---|---|
| English | 4.5 | 5.1 | 4.1 | 4.5 |
| German | 4.2 | 4.8 | | 5.2 |
| French | 4.8 | 5.9 | | 5.9 |
| Spanish | 2.9 | 3.3 | | 3.1 |
| Italian | 2.4 | 2.9 | | 3.1 |
| Dutch | 6.9 | 7.3 | | 6.2 |
| Portuguese | 4.5 | 5.0 | | 4.4 |
| Polish | 6.9 | 7.7 | | 6.2 |

Whisper base.en and tiny.en scored 8.3 and 10.1 on English. The Parakeet models ran at a median of 160 to 200 times real time, Whisper large-v3-turbo at 60 to 86 times.

The catalogue already used `recommended_for` for the one-line reason first run shows beside a model, so the per-language scores needed their own key.

## Decision

- Each speech model in `crates/dettivo-speech/catalogue/v1.toml` carries one `[[models.language_wer]]` table per scored language with `language`, `rank` and `wer`, copied from the recorded run. A new model joins the ranking by running the step with it on disk.
- The step ranks per language. Rates less than 1.5 points apart tie, and the faster model wins a tie, measured as the median realtime factor over the model's languages. Models within 10% of the fastest count as equally fast, and the lower rate decides between them, so Parakeet v3 and Parakeet Ultra, which share one architecture, never rank on timing noise.
- `speech.providers.list` takes an optional `language` (a code or a locale) and then answers `language_ranking` with each model's rank, rate and readiness. `dettivo speech providers --language <code>` prints the recommendation and the ranking.
- First run asks for the ranking of `[dictation] language`, or the system locale's language while that is `auto`. The first model is marked recommended with its rate, and while no speech model is ready, no download runs and nothing was picked, it is the selected row. First run writes nothing until the person picks, and a language the run did not cover keeps the GPU-tier shortlist.

## Consequences

- From this run, English recommends Parakeet v2, and German, French, Spanish, Italian, Dutch, Portuguese and Polish recommend Parakeet Ultra. Whisper large-v3-turbo has the lowest rate in Dutch, Portuguese and Polish, but within 1.5 points of Parakeet Ultra at under half its speed.
- 100 utterances per language is about 1,800 to 2,400 reference words. A difference under about one point is within what another 100 utterances could move, which is one reason for the 1.5-point tie.
- Only the eight languages of the run are ranked. Another language, or a locale such as `C`, falls back to the shortlist first run showed before.
- The ranking is for GPU speed. On the CPU tier first run still recommends by rate and speed measured on Vulkan.
