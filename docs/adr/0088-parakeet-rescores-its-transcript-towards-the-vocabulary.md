# 0088. Parakeet rescores its transcript towards the vocabulary, and a term replaces heard words only where the audio supports it

Status: Accepted 2026-11-01, amends 0086

## What this gives you

On Parakeet, the default dictation engine, `[dictation] vocabulary` now changes what you get: a name the model misheard comes back spelt as you listed it when the audio fits the name, and ordinary speech is left alone. `[dictation] vocabulary_boost` (`off`, `light`, `normal`, `strong`) sets how far the vocabulary may overrule the model.

## Situation

Parakeet accepted the Whisper prompt and dropped it, so a Parakeet user got nothing from a vocabulary. Parakeet is a token-and-duration transducer (TDT): an encoder over 80 ms audio frames, a prediction network over the tokens emitted so far, and a joint network that scores a token and a duration at a frame. Greedy decoding keeps the best token frame by frame, which is where a coined name turns into the nearest known word (`Hyprland` into `Hyperland`, `Dettivo` into `dativo`).

The pinned parakeet.cpp 0.5.0 exposes no encoder frames, decoder state or joint network through its C API. Its private headers do: `Encoder`, `PredictionNet`, `Joint` and `transducer_greedy_batch` are public classes and functions in `src/`, and `parakeet-cpp-sys` already compiles a shim against them.

Two designs were on the table. Stage 1 rescores after the greedy decode. Stage 2 carries shadow hypotheses through a token trie during the decode, and was to be built only if stage 1 missed the recall target on the benchmark (ADR 0087). ADR 0089 records the stage 2 decision.

## Decision

- `recognize` carries `vocabulary: { terms: [{ term, aliases }], boost }` for Parakeet. The daemon fills it for a dictation on Parakeet from `[dictation] vocabulary` (each term with its `sounds_like` spellings) and the boost level, and sends none when the list is empty or the level is `off`. Whisper keeps its prompt and never gets the field. File transcription and meetings send none.
- The levels are nats added per token piece of a term: `off` 0, `light` 2.0, `normal` 2.5 (default), `strong` 4.0 (`VocabularyBias::boost_of`).
- With a vocabulary the engine decodes each chunk through `parakeet_sys_take_open` (`crates/parakeet-cpp-sys/bias.cpp`), which runs the calls `parakeet_capi_transcribe_pcm_batch_json_lang` runs for a batch of one and keeps the encoder output and the greedy tokens with their frames and durations. Without one, or with the level `off`, the engine calls the C API exactly as before.
- A candidate is a run of one to four heard words that matches a term or one of its spellings in one of four ways: it equals a spelling; the longest common subsequence covers three quarters of the longer string; the edit distance is at most a quarter of the spelling's length (at least 1); or, for a term of five sounds or more, its consonant-sound key is one edit from the term's (`crates/dettivo-engine-parakeet/src/bias/sound.rs`). Each word of a longer run must bring it closer to the term, so a run never swallows a neighbouring word (`to Sapien XT` stays a candidate only as `Sapien XT`).
- Never candidates: the term in another case (`dettivo` for `Dettivo`), the term or a spelling plus `s` or `es` (plurals and possessives), any run overlapping words that already are the term (so `because of the` heard right never becomes `because of the the`), any word with `/`, `\`, `@`, `~`, `://` or a dot between letters (paths, URLs, addresses), a word that opens with a mark, and a term under five letters with no spelling. A term longer than 12 token pieces is skipped, and a take scores at most 16 candidates, closest first.
- Each candidate is scored twice by `parakeet_sys_take_score`, over the frames from where greedy decoding resumed after the previous token (at most 4 frames before the span) to the next token's frame (at most 4 frames after the span), from the decoder state greedy decoding had there: once forced to the heard tokens, once to the term in its fewest pieces. The score is the TDT forward algorithm, summing every alignment, with the token and duration distributions log-softmaxed apart and no zero-duration blank.
- The term replaces the run when `log P(term) + boost x pieces(term) >= log P(heard)`. Overlapping winners keep the larger margin. The new word keeps the run's first start, last end, lowest confidence and the last word's trailing punctuation, unless the term ends in a mark of its own.
- The engine logs `vocabulary rescoring candidates=<n> replaced=<n> rescore_ms=<ms>` per request and the per-candidate scores at debug level, never a word.

## Consequences

- A distractor never came close on the benchmark: `clay`, `Omar` and `sapient` scored 24 to 47 nats short of the term even at `strong`, because the term's tokens explain audio that does not contain them poorly. The guard sentences produced no candidate.
- Misheard coined names score 2 to 9 nats short at `normal`. `normal` recovers 5 of the 12 terms Parakeet missed and `strong` 8 ([vocabulary-2026-11-01-thor-rescoring](../reports/benchmarks/vocabulary-2026-11-01-thor-rescoring.md); ADR 0089 has the margins). The transducer's own spelling knowledge keeps `Hyprland` behind `Hyperland`, and the boost level is the lever a user has.
- Rescoring added a median of 15.0 ms to a clip with a candidate and at most 1.4 ms to a clip without one, release build on Vulkan, the vocabulary's preparation included. With 500 terms of three 64-letter aliases each, a take without a candidate gained 8 ms.
- Scoring the term in lower case or with a capital, or summing over every segmentation of it, moved no clip; the fewest-pieces spelling scored highest in every case measured.
- A take with no candidate pays for one copy of the encoder output and the string matching; the encoder projection is computed only for the first scored candidate.
- The engine groups the greedy tokens into words itself and checks the grouping against the library's words; a transcript it cannot match is left as decoded. The shim declares `struct parakeet_ctx` as parakeet.cpp 0.5.0 defines it, so a parakeet.cpp upgrade must re-check that definition and `subsampling_tile`, which mirrors a static helper in `model.cpp`.
- Settings has no control for the level yet; it is set in the file or with `dettivo config set dictation.vocabulary_boost strong`.
