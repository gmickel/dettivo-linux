# Keep the dictated language when a vocabulary is set

## Goal & Context

Issue #37: with `[dictation] vocabulary` set, some English dictations with Whisper `large-v3-turbo` came out as fluent German (or Japanese), with `language` unset and even with `language = "en"`. `text_raw` is already in the wrong language, so Whisper produced it, not the polish pass.

The daemon passes the vocabulary to Whisper as its initial prompt, built by `vocabulary.join(", ")` in five places: live dictation (`dettivo-session/src/worker.rs`), history re-run (`dettivod/src/jobs.rs`), retry (`dettivod/src/jobs_retry.rs`), meetings (`dettivod/src/meetings.rs`) and imports (`dettivod/src/imports.rs`). The result is a comma list with no closing punctuation. The reporter measured that this unterminated list pulls the decoder off the spoken language, and that ending it with a full stop fixes it: over 103 English dictations, the bare list changed the language of 3 takes against no prompt, and the terminated list changed none.

This spec builds the vocabulary prompt in one function, ending it with a full stop, and uses it at every call site.

## Acceptance

- **R1:** One function in `dettivo-speech` builds the Whisper prompt from the vocabulary: `None` for an empty list, otherwise the words joined with `", "` and ending in `.`, unless the last word already ends in `.`, `!` or `?`.
- **R2:** Live dictation, history re-run, retry, meetings and imports all build their `RecognizeRequest`/meeting prompt with that function; no call site joins the vocabulary itself.
- **R3:** Tests: unit tests for the function (empty, one word, several words, a last word already ending in `.`, `!` or `?`), each failing on the old join.
- **R4:** `docs/engines.md` states how the vocabulary reaches Whisper; `CHANGELOG.md` gets an Unreleased Fixed entry crediting the reporter.

## Boundaries

- No label or sentence around the words (`Vocabulary: ...`): it adds English to every prompt and could pull non-English dictation toward English.
- No change to how Parakeet drops the prompt, to the Enhanced LLM prompt, or to the stored vocabulary.
- No new config key.

## Decision Context

Reported by @DanielKillenberger in #37 with measurements on 0.4.3 and a reference change on their fork. `CONTRIBUTING.md` takes fixes as descriptions, so the fix is written here.
