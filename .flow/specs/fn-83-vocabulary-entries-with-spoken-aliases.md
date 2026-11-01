# Vocabulary entries with spoken aliases and a term-recall benchmark

## Goal & Context

A vocabulary is how users teach Dettivo their names and jargon. Today `[dictation] vocabulary` is a flat list of words. It can't say how a term tends to be misheard, and nothing measures whether the vocabulary actually helps. The Enhanced prompt receives the words but not their likely mishearings. The Parakeet vocabulary work (next spec) needs both aliases and a measurement.

This spec makes each vocabulary entry carry optional "sounds like" spellings, feeds them to Enhanced, and adds a term-recall benchmark to the QA rig.

## Approach

**Schema.** An entry is either a plain string, as today, or a table. Both forms are accepted, and a plain string means `{ term = "...", sounds_like = [] }`.

```toml
[dictation]
vocabulary = [
  "Dettivo",
  { term = "Clef", sounds_like = ["clay", "cleff"] },
  { term = "SapienXT", sounds_like = ["sapient x t"] },
]
```

```rust
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum VocabularyEntry { Word(String), Entry { term: String, #[serde(default)] sounds_like: Vec<String> } }

impl VocabularyEntry {
    pub fn term(&self) -> &str { /* ... */ }
    pub fn aliases(&self) -> &[String] { /* ... */ }
}
```

**Limits.**
- 500 entries, 64 characters per term, 16 aliases per term.
- Duplicates are removed case-insensitively, and the first spelling wins.
- The config editor, the CLI (`dettivo config set dictation.vocabulary`) and the GUI accept both forms. The GUI edits aliases per entry.

**Contract parity.** The contract's vocabulary stays a list of strings for clients that only know that shape. Aliases are a Linux addition listed in the documented delta.

**Whisper prompt.** Unchanged: terms only, never aliases. Built by `vocabulary_prompt` with its cap and terminator.

**Enhanced.** The vocabulary block of the system prompt names aliases:

```text
Vocabulary (spell these exactly when the speaker means them): Clef (may be transcribed as clay, cleff); SapienXT (may be transcribed as sapient x t); Dettivo
```

**Term-recall benchmark in the QA rig.**
- Build a fixture set:
  - target clips: each term in three natural sentences;
  - distractor clips: sound-alike words that must not become the term;
  - a general-speech guard set with no terms.
- Generate the clips with the rig's TTS voices, or record them, with licences noted.
- `dettivo-qa bench vocabulary` reports, per engine and model and with the vocabulary on and off:
  - term recall;
  - false insertions on distractor and guard clips;
  - WER on the guard set.
- Matching is whole-word and case-insensitive, with aliases never counted as hits.

```rust
pub struct VocabularyScore { pub term_recall: (u32, u32), pub false_insertions: u32, pub guard_wer: f32 }
```

## Acceptance

- **R1:** `[dictation] vocabulary` accepts plain strings and `{ term, sounds_like }` tables in one list. Existing configs load unchanged, and round-trips through `config set` keep comments and form.
- **R2:** Limits are enforced with clear errors naming the entry. Duplicates are removed case-insensitively.
- **R3:** The Enhanced system prompt carries each term with its aliases in the format above. A unit test pins the format.
- **R4:** The Whisper prompt contains terms only.
- **R5:** The GUI vocabulary editor shows and edits aliases. The CLI accepts both forms.
- **R6:** `dettivo-qa bench vocabulary` runs the fixture set and writes a report with term recall, false insertions and guard WER per engine and model, vocabulary on and off. A baseline run is recorded in the benchmark report.
- **R7:** `docs/dictation.md`, `docs/config.md` and the contract delta list describe the entry form. A changelog entry is added.

## Boundaries

- Aliases never rewrite text by themselves. They inform the engine bias and the Enhanced prompt; `replacements` stays the blunt tool.
- The fixture set is synthetic or self-recorded. No personal dictation audio enters the repository.

## Quick commands

- `cargo test -p dettivo-core -p dettivo-language -p dettivo-qa`
- `just build test lint`
