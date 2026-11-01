# 0086. A vocabulary entry names how its term is misheard, and only Enhanced reads the aliases

Status: Accepted 2026-11-01

## What this gives you

You can tell Dettivo that "Clef" tends to come back as "clay" or "cleff", next to the term itself in `[dictation] vocabulary`. The Enhanced rewrite reads those spellings and corrects them to the term when the speaker meant it. Your existing list of plain words loads and works as before.

## Situation

`[dictation] vocabulary` was a list of strings. Whisper takes the list as its initial prompt (ADR 0081 caps it at 500 characters of whole terms), and the Enhanced system prompt carried it as a JSON array with a spelling hint. Neither engine nor model learned how a term is actually misheard. Parakeet biasing (fn-84) needs a misheard form to bias towards the term, and the term-recall benchmark (ADR 0087) needs one to tell a near miss from a miss.

Two shapes were on the table: a separate `[dictation] aliases` table keyed by term, or entries that carry their own aliases. A separate table can drift from the list, so a removed term would leave orphan aliases behind.

## Decision

- An entry is a string or a `{ term, sounds_like }` inline table, mixed in one list. A string means `{ term = "...", sounds_like = [] }`. Any other key in the table is refused by name (`unknown field`), so a misspelt `sound_like` never drops its aliases unseen.
- The limits are 500 entries, 64 characters a term and 16 `sounds_like` spellings a term. A file, `dettivo config set` and the GUI that break one are refused with `dictation.vocabulary` as the key and the offending entry in the message.
- A later entry whose term matches an earlier one regardless of case is dropped as the file loads. The first spelling wins, with its aliases.
- The Whisper prompt and the prompt-echo guard take the terms only (`vocabulary_schema::terms`). An alias in the Whisper prompt would teach the engine the misspelling.
- The Enhanced system prompt ends with one line, the first 50 entries joined by `; `:

  ```text
  Vocabulary (spell these exactly when the speaker means them): Clef (may be transcribed as clay, cleff); SapienXT (may be transcribed as sapient x t); Dettivo
  ```

- An alias never rewrites text by itself. `[dictation] replacements` stays the tool for a fixed substitution.
- The contract's vocabulary stays a list of strings. A plain entry still reads back as a string through `config.get`, and a table entry is a Linux addition listed in `docs/api/linux-deltas.md`.

## Consequences

- `VocabularyEntry` in `dettivo-core` is the one type every consumer reads: the session policy, the Enhanced prompt, the import, job, retry and meeting prompts. fn-84 adds the Parakeet bias on the same type.
- A JSON object arrives at `config.set` with sorted keys. `config set` writes `term` first, so a table written from the GUI reads `{ term = "Clef", sounds_like = [...] }` like the documented form.
- The Enhanced prompt changed shape: the `<vocabulary hint="spelling">` JSON block is gone. Polish evaluations recorded before this change ran with the old block.
- A client that only knows a list of strings sees an object for a table entry in `config.get`. It still sees every plain entry unchanged.
