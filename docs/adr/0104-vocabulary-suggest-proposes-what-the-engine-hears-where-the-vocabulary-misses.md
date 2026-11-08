# 0104. Vocabulary suggest proposes what the engine hears where the vocabulary misses

Status: Accepted 2026-11-08

## What this gives you

`dettivo vocabulary suggest` tells you which of your terms the speech engine you dictate with already writes right, and proposes `sounds_like` spellings for the rest, so you no longer guess how Parakeet or Whisper mishears a name. On the term-recall bench the spellings it generated left recall where it was without them for Parakeet (13 of 15) and Whisper (10 of 15), raised Whisper's names said alone from 6 to 7 of 10, and added no false insertion.

## Situation

A `sounds_like` spelling feeds Parakeet's candidate search (ADR 0088) and Whisper's check of a lone name (ADR 0095), but only the person typing the vocabulary could supply one. The rig already speaks terms with two Piper voices for the term-recall bench (ADR 0087), so the engine can be asked how it hears a term directly.

The first version transcribed each clip with no vocabulary and proposed every misheard stretch. Measured on the bench, Whisper's spellings held recall, but Parakeet's cost a target: with `amarky`, `amarkey` or `amarke` listed for Omarchy, Parakeet's candidate search picked the shorter span `Markey` in "O Markey ships with a tiling window manager" and the rescoring left it, so recall fell from 13 to 12 of 15 ([terms only](../reports/benchmarks/vocabulary-2026-11-08-thor-terms-only.md)). Running Parakeet on that clip with each spelling alone showed those three each stop the replacement and `amarchy` keep it. A spelling the vocabulary already covers adds nothing and can move the candidate search off the span it would have rescored.

## Decision

- The daemon's `speech.vocabulary.suggest { term }` has `piper` speak the term alone and in "I think {term} is great." with `en_US-amy-low` and `en_US-ryan-low`, noise scales 0 so each voice says a text the same way every run. It looks for an executable `piper` on its `PATH` and then in `~/.local/bin`, and for each voice's `.onnx` and `.onnx.json` in `<models_dir>/piper/`. A missing piece is named with `pipx install piper-tts` and the voice download page, and nothing runs.
- The selected dictation engine transcribes each clip twice, padded and guarded as a take is (`pad_for_engine` with its unpadded retry for an empty result, `guards::clean`): once with the vocabulary a take carries, with the term added when the vocabulary lacks it, and once with none.
- A clip is right when the first text holds the term as a whole word in any case, the bench's rule, and a term spelt with marks (`C#`, `C++`) only where the text keeps them. A term whose four clips are right needs no `sounds_like`. In a clip that is still wrong, the spelling is the second text's words where the term was: the whole text for the term alone, the words between "I think" and "is great" for the sentence, where the right term is also looked for only between them; a sentence that did not come back is neither right nor a spelling. A sentence that did not come back, a stretch more than two words longer than the term, and a spelling the entry already lists are not proposed.
- The CLI asks once per term and writes only with `--apply`, through `config.vocabulary.add` with `extend` (ADR 0101): every term heard goes in, so a term the file lacks is appended, with its spellings or alone, because its clips were heard with it in the vocabulary, and an entry the file holds gains the spellings it lacks in place, a plain term turning into `{ term, sounds_like }` beside its comment. Without `--apply` the same call runs as a dry run, and the CLI prints what it would write.
- `dettivo-qa bench vocabulary --vocabulary <file>` primes the engines with a JSON list of entries in place of the set's own, so a generated vocabulary is measured on the same clips.

## Consequences

- Measured on thor at `12c131e` with the Vulkan engines, the set's five terms as plain entries in a test daemon. Suggest flags SapienXT, Omarchy and Hyprland as right on Parakeet v3 and proposes 3 spellings each for Dettivo and Clef; on Whisper large-v3-turbo it flags SapienXT and Omarchy and proposes 4 for Dettivo, 1 for Clef and 2 for Hyprland. Two runs in fresh daemons gave byte-identical output for each engine, in 26 to 37 s for the five terms.
- On the bench ([Parakeet's spellings](../reports/benchmarks/vocabulary-2026-11-08-thor-suggested-parakeet.md), [Whisper's](../reports/benchmarks/vocabulary-2026-11-08-thor-suggested-whisper.md)), recall with the vocabulary on is 13 of 15 for Parakeet and 10 of 15 for Whisper, as with the terms alone, with 0 false insertions in the distractor and guard clips. Whisper's single names go from 6 to 7 of 10 because `blah` keeps Clef for "Blah.", with 0 sound-alikes said alone becoming a term.
- The spellings belong to the engine that heard them. Whisper primed with Parakeet's spellings wrote Clef for a lone "Clay." (`cleft`, `where` and `left` are listed), one false insertion among the single names. `--apply` only adds spellings, so after changing the dictation engine you remove the previous engine's spellings before running suggest again.
- Real words get proposed (`left`, `where`, `the devil`), because they are what the engine wrote. The bench found no insertion from them on either engine's own path, since both engines check a candidate against the audio, but the set has no distractor sentences built around those words.
- Piper reads a coined term through espeak-ng as it is spelt ("Hyprland", not "Hyper land"), so the clips are one accent's guess and the spellings are an upper bound for it, as ADR 0087 notes for the bench. The voices are English; the engine transcribes with `[dictation] language`.
- A term costs eight transcriptions and four Piper starts of about a second each, so a 500-entry vocabulary takes tens of minutes. The CLI waits up to 10 minutes per term.
- The bench reports record `12c131e`, the commit the work started from: the bench code and the engines are that commit's plus the `--vocabulary` flag, and the generated vocabulary each run used is in its report.
