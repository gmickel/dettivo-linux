# 0062. A cut-off analysis answer splits the part, and the reduce joins the lists in code

Status: Accepted 2026-09-16, amends 0036 (the map and the reduce of the analysis)

## What this gives you

A long meeting analysed by the local model on the default 4096-token context now ends with a summary, the decisions and the action items instead of `invalid_output`. The run takes more model calls than before on such a meeting (the retained 52-minute dogfood transcript needed 15 calls in the prototype where 6 had failed), and every decision or action item a part produced reaches the result, because the model is never asked to write the lists back out.

## Situation

The 2026-09-16 dogfood meeting (spec fn-63) finalised 948 segments, and the automatic analysis failed with `invalid_output: no JSON object in the output; after the repair pass: no JSON object in the output`. The daemon cut the transcript into five parts of at most 12000 characters (`[meetings.analysis] chunk_chars`) and asked `qwen3-4b-instruct-2507` through `dettivo-engine-llm` with `[engines.llm] context_length = 4096` and `max_tokens = 1024`. Parts one to four answered valid JSON. Part five's prompt left 243 tokens of context, the engine's budget is the smaller of `max_tokens` and what the prompt leaves (`crates/dettivo-engine-llm/src/engine.rs`), so the answer stopped before its closing brace with `finish_reason = length`. `LocalLlm::generate` returned only the text, the analysis could not tell a cut-off answer from a wrong one, and the repair pass added the invalid answer to the system prompt, which left even fewer tokens. Running the installed daemon against an isolated copy of the meeting reproduced the same failure in 22.8 seconds.

A first prototype split the failing part and merged the parts pairwise through the model's full JSON shape. It failed at the last merge, where the combined decisions and action items alone exceeded the output budget. That result is kept as evidence that a merge that asks the model to rewrite growing lists fails on the same budget the parts fail on.

## Decision

**The provider reports how an answer ended.** `LlmProvider::answer` returns an `Answer` with a `Finish` (`Stop`, `Length`, `Unknown`); the default forwards `rewrite` with `Unknown`, so Ollama, an endpoint and the mock keep their behaviour. The daemon's `LocalLlm` maps the engine's `finish_reason` (`length` becomes `Length`; `cancelled` is no answer and becomes the same provider error an engine cancel raises, so a cancelled generation never splits or retries anything) and maps the engine's `bad_request` for a prompt that exceeds the context onto a new `ProviderError::PromptTooLong`. The Enhanced pass treats that error as a rewrite failure and falls back.

**A cut-off part is halved, never repaired.** The analysis treats an answer as cut off when the provider says `Length` or when the text opens a JSON object that never closes (`parse::cut_off`). Such a part, and a part whose prompt did not fit, is halved on its middle line (inside a single line, at the nearest space before its middle) and each half is analysed on its own, at most `MAX_SPLITS = 4` levels deep, so one part becomes at most sixteen pieces. A piece still cut off at that depth fails the run with `invalid_output` naming the split count and the two settings that would fit. The repair pass stays for an answer that finished and did not parse.

**The reduce asks the model for one summary and joins the lists in code.** `reduce::combine` concatenates the parts' decisions and action items in order and drops exact repeats after whitespace and case folding and trailing sentence punctuation (an action item repeats only when its text, owner and due all match). The model receives only the parts' summaries, under `MERGE_SYSTEM_PROMPT`, in groups of at least two summaries and at most `chunk_chars` characters per call, repeated until one summary remains. A merge call the budget cuts off, or whose prompt does not fit the context, lowers that limit under the group and combines the group in smaller groups instead (a dense language or many parts can put more tokens in `chunk_chars` characters than the context holds); a pair that still does not fit fails the run naming that. Every call still checks the run's cancel flag and deadline first.

## Consequences

- The retained transcript (948 segments, 59861 characters, five parts) ran through this code against the installed engine and model in an isolated process: 8 calls, 6 pieces, 18.5 seconds, the fifth part cut off at 243 tokens and its two halves answered, one merge call over the six summaries, 16 decisions and 23 action items (counts, not checked against the meeting). The whole run still shares `[meetings.analysis] timeout_ms` (60000 by default).
- `Outcome.parts` counts the pieces analysed, splits included; `meeting analysis ready` logs it.
- The merge prompt lists summaries under `Part N:` labels, so a fixture that answered the old full-JSON merge still parses (its summary is read, its lists are ignored). The golden suite in `crates/dettivo-language/tests/goldens/meeting_analysis.json` stages a `length` finish (`__LENGTH__:` prefix) and a too-long prompt (`__TOO_LONG__`) through the scripted provider.
- The default `chunk_chars` of 12000 stays. On the 4096-token local context a part of that size overflows for dense or non-English text, and the split absorbs it at the price of extra calls; a smaller `chunk_chars` avoids the wasted call.
- Duplicate removal is exact after folding. Two wordings of one decision stay two decisions; the model is no longer asked to judge that, so the lists can be longer than a model-merged list would have been.
