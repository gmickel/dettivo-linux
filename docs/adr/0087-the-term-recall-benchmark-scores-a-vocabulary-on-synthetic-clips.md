# 0087. The term-recall benchmark scores a vocabulary on synthetic clips, with the vocabulary off and on

Status: Accepted 2026-11-01

## What this gives you

`dettivo-qa bench vocabulary` tells you, per engine and model, how many of your terms come back spelt right with the vocabulary off and on, how often a term appears where nobody said it, and whether the vocabulary costs accuracy on ordinary speech. A change to vocabulary handling, such as Parakeet biasing in fn-84, is judged by these numbers instead of by a few dictations.

## Situation

Nothing measured whether `[dictation] vocabulary` helps. The NFR bench (ADR 0029) times the engines on one clip with no vocabulary. A vocabulary can also hurt: a primed engine may write a term over a word that only sounds like it, or drift on speech that has no term in it. A useful measure needs clips that carry the terms, clips that carry sound-alikes, and clips with neither.

The fixtures cannot be personal dictation audio. The rig already synthesises speech with two Piper voices through sherpa-onnx's offline TTS for the diarization and meetings fixtures (ADR 0035, ADR 0039), so the same voices render this set.

## Decision

- The set lives in `crates/dettivo-qa/fixtures/vocabulary/`. `set.json` holds five vocabulary entries with their `sounds_like` spellings and 24 clips: 15 targets (each term in three sentences), five distractors (a sound-alike word such as `clay` or `detective`) and four guard sentences with no term. Each clip has the text the voice reads (`say`, `Sapien X T` for SapienXT) and the text that was meant. `scripts/qa/make-vocabulary-fixture.py` renders the WAVs, 16 kHz mono, and the voices alternate between Piper `en_US-amy-low` and `en_US-ryan-low`.
- Each engine's CLI mode transcribes every clip twice per model, with the vocabulary off and on. On means the Whisper `--prompt` built the way `dettivo_speech::vocabulary_prompt` builds it, terms only. An engine without a vocabulary input records its on row as skipped with the reason. Parakeet is one until fn-84 adds the bias.
- A term is a hit only as a whole-word, case-insensitive match of its own spelling. `Sapien XT` for `SapienXT` and every `sounds_like` spelling are misses.
- The report files as `vocabulary-<date>-<host>.json` and `.md` under `docs/reports/benchmarks/`. The name ends in neither `-gpu.json` nor `-cpu.json`, so the NFR README never reads it.

## Consequences

- The baseline on 2026-11-01 ([vocabulary-2026-11-01-thor](../reports/benchmarks/vocabulary-2026-11-01-thor.md), CPU engines at `9d08431`) is the bar fn-84 is measured against. Whisper large-v3-turbo recalls 2 of 15 terms with the vocabulary off and 10 of 15 with it on. Parakeet v3 recalls 3 of 15 and takes no vocabulary yet. The guard word error rate is 0 in every row.
- The vocabulary costs Whisper two false insertions, both in one clip. "Omar keeps the spare keys in his desk." came back as `Omarchy, Hyprland.`, the tail of the prompt. The session's echo guard (ADR 0081) drops a segment only when it echoes the whole prompt, so this partial echo would reach a paste. It is recorded here, not fixed.
- Whisper still misses `Dettivo` with the vocabulary on (`Detivo`, `Ittilo`, `Didivo`) and writes `Hyperland` for two of three Hyprland clips. Those are the spellings a `sounds_like` list exists for.

- The voices read the terms through espeak-ng, which pronounces a coined word one way every time. A real speaker varies more, so recall here is an upper bound for one accent. The set measures relative change between vocabulary off and on, and between engines.
- The ryan voice's dataset is CC BY-NC-SA 4.0, as for the diarization and meetings fixtures. A set that must avoid it re-renders with amy alone by editing `voice` in `set.json`.
- The Enhanced pass is outside the bench: the clips go through the engines only, so the aliases, which only Enhanced reads (ADR 0086), are not measured yet.
