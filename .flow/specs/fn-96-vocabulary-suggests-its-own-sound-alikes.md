# Vocabulary suggests its own sound-alikes

## Goal & Context

A vocabulary term helps most when Dettivo knows how the engine mishears it: fn-83's `sounds_like` spellings feed fn-84's and fn-89's checks. Today a user has to guess them. The engine can tell us instead: speak each term with two TTS voices and transcribe it. A term the engine already gets right needs no entry, and the spellings it produces for a mangled term are its sound-alikes. Dettivo already renders Piper TTS clips for the term-recall bench (`scripts/qa/make-vocabulary-fixture.py`, ADR 0087).

This spec adds `dettivo vocabulary suggest`, which hears each term the way the engine will and proposes `sounds_like` entries, so recall improves without the user guessing.

Origin: Gordon, 2026-11-06: a follow-up for later, which pairs with fn-95's vocabulary import.

## Approach

- For each term, or the whole vocabulary, render it alone and in a short carrier sentence with two Piper voices, transcribe with the selected engine, and report whether it was recognised or how it was misheard.
- Propose the misheard spellings as `sounds_like` entries, and write them only with `--apply`, through the config edit path.
- Piper and its voices are an optional download like other models; without them the command says what to install.

## Acceptance

- **R1:** On ADR 0087's term set, the command flags the terms the engine already gets right and proposes sound-alikes for the rest; output is deterministic for a fixed engine and voice set.
- **R2:** On the term-recall bench, recall with the generated `sounds_like` is at least recall without them, with zero new false insertions on the distractor clips (fn-84's and fn-89's bar), for Parakeet and Whisper.
- **R3:** Nothing is written without `--apply`; a test covers it.
- **R4:** Docs describe the command; an ADR records the method and the bench result; a changelog entry is added.

## Boundaries

- Runs fully locally; nothing leaves the machine.
- No vocabulary mining from History or from edits after a paste.

## Quick commands

- `cargo test -p dettivo-cli -p dettivo-qa`
- `dettivo-qa bench vocabulary`
- `just build test lint`
