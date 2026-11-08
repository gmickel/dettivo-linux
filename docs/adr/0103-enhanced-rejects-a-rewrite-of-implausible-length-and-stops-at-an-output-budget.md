# 0103. Enhanced rejects a rewrite of implausible length and stops a local generation at an output budget

Status: Accepted 2026-11-08

## What this gives you

An Enhanced rewrite that balloons past twice your words, collapses to a fraction of them, or quietly drops a `not` never reaches your text: the deterministic Polish text goes in instead. A local generation that would run on stops at 1.5 times the dictation's tokens plus 32, so a runaway costs a fraction of a second instead of the 1024 tokens `[engines.llm] max_tokens` allows. On the private eval sets the protected, required and forbidden metrics did not move.

## Situation

Three simple rules catch whole classes of model failure: reject an output over 2x the input's word count, or under 0.4x once the input passes 8 words; stop generation at 1.5x the input tokens plus 32; reject a short deletion of 2 words or fewer that removes only meaningful words. The output guard (`crates/dettivo-language/src/enhanced/guard.rs`, ported from the macOS `PolishOutputGuard`) already had length checks of its own, so each rule was compared with what it did:

| Rule | What the guard had | The gap, with an input that slipped through |
|---|---|---|
| Over 2x the words | a character cap, `max(chars + 40, chars x 1.8)`, only for a dictation of 24 characters or more | `ship it tomorrow` (3 words, 16 characters) rewritten as `Ship it tomorrow, and tell everyone you know.` (8 words) |
| Under 0.4x past 8 words | a contraction rule for non-technical input that rejects 3 or more dropped words only when the rewrite does not end a sentence | a 15-word dictation about moving a launch review rewritten as `Move the review.` |
| Output budget | none: every Enhanced request asked for `[engines.llm] max_tokens`, 1024 by default, and the provider's finish reason was never read | a generation that never ends its turn ran until 1024 tokens or `[llm] timeout_ms` |
| Short deletion of meaningful words | a check for questions, repair cues and repeated lists that needs 2 or more dropped words of 3 or more letters outside its stop-word list, and `not` is a stop word | `we should not ship the release on Friday` rewritten as `We should ship the release on Friday.` |

The Qwen3 tokenizer, run over the 431 transcripts of the private eval sets, puts a median of 0.22 tokens on a character and at most 0.49, and spends 14 tokens on each output marker the prompt requires.

## Decision

All four gaps close, in `guard/length.rs` beside the guard.

**Length.** A rewrite with more than twice the dictation's words fails with the guard's expansion reason. One with under 0.4 of them, when the dictation has more than eight, fails with its contraction reason, so the Enhanced pass inserts the deterministic text without a repair pass when that text passes the guard. Technical dictation is exempt from the second rule, as from the contraction rule: a spoken path (`dettivo slash services slash ... dot swift`, 13 words) rightly becomes one token, and the first eval run rejected exactly that row until the exemption went in. The length rules run after the refusal and input-request checks, so a long refusal keeps its more specific reason.

**Short deletion.** A rewrite that is the deterministic draft with one or two words taken out, and nothing else changed but casing and punctuation, fails with `output dropped the dictated word <words>` and the draft goes in. The draft is the reference instead of the dictation because the deterministic transforms drop words of their own (`R and D` becomes `R&D`). Fillers, hedges and intensifiers (`um`, `really`, `probably`, the parts of `you know` and `kind of`) may go, and so may a word said twice and kept once. A deletion of three or more words is outside the rule.

**Output budget.** `RewriteRequest` carries `max_tokens`, and the Enhanced pass sets it to 1.5 times the dictation's tokens plus 32, plus 32 for the output markers. The tokens are estimated at one per three characters, which the tokenizer stays under on 421 of the 431 transcripts, so the estimate errs long and never cuts a sound rewrite short. The daemon's local engine generates at most that many tokens and never more than `[engines.llm] max_tokens`. When the engine reports `finish_reason = length`, the pass falls back at once with `guard_rejected` and `the rewrite ran past its output budget`, without a repair pass, since a repair would run to the same budget again. Ollama and an OpenAI-compatible endpoint keep their own limits; neither reports a finish reason the pass reads.

## Consequences

`dettivo-qa polish-eval` measured the change on an RTX 4090 over Vulkan with `qwen3-4b-instruct-2507` (the default) and `qwen3-1.7b`, on the two private held-out sets (10 rows each) and on the 275 rows of the private corpus v3 (`--split all`). The baseline ran twice to measure run-to-run spread. Medians on the same code moved by up to 14 ms between those two runs.

| Model, set | Protected | Required | Forbidden | Guard rejections | Fallbacks | Median ms (baseline runs, after) |
|---|---|---|---|---|---|---|
| 4B, whisper held-out | 60.0% = | 20.0% = | 0.0% = | 20.0% to 20.0% | 20.0% to 20.0% | 387, 390 to 388 |
| 4B, parakeet held-out | 60.0% = | 30.0% = | 0.0% = | 20.0% to 20.0% | 20.0% to 20.0% | 383, 397 to 382 |
| 4B, corpus v3 | 84.4% = | 58.9% = | 0.73% = | 8.0% to 8.4% | 8.0% to 8.4% | 375, 367 to 383 |
| 1.7B, whisper held-out | 60.0% = | 20.0% = | 0.0% = | 20.0% to 20.0% | 20.0% to 20.0% | 221, 230 to 221 |
| 1.7B, parakeet held-out | 60.0% = | 30.0% = | 0.0% = | 20.0% to 20.0% | 20.0% to 20.0% | 220, 228 to 229 |
| 1.7B, corpus v3 | 83.3% = | 63.6% = | 0.73% = | 20.0% to 20.7% | 20.0% to 20.7% | 234, 245 to 250 |

Exact match rose on both corpus runs, from 18.2% to 18.9% on the 4B and from 29.8% to 30.2% on the 1.7B. Ten rows changed outcome, and in each one the inserted text moved toward the gold. The short-deletion rule caught the 1.7B dropping German `nicht` from `Bitte ändere SettingsView.swift nicht` in two rows, which turned "do not change" into "change". In the eight 4B rows it caught a dropped `please`, `that`, `those`, `as well`, `blah`, `some` and `Roadmap:`, and in each the gold keeps the word. The guard rejection and fallback rates rise by the rows whose draft equals the dictation: there the pass cannot insert the draft silently and runs its repair passes before the notice.

The output budget never fired on the 570 rows. No eval row runs away, and the 9 to 14 s p95 of the held-out sets is the first row that reaches the model, which pays for the engine's load. On rows whose outcome did not change, the paired per-row median moved between -14 and +3 ms. The held-out medians moved by -15 to +1 ms against the second baseline run. The 4B corpus median rose 8 to 16 ms against the two baseline runs, which is at the edge of the 14 ms run-to-run spread. The paired comparison puts 3 ms of that on unchanged rows, and the rest on the rows the new rules sent through repair passes.

The tests prove each rule where it applies. `guard::length` tests the four rejections, the inputs each existing rule let through, and the cases that must pass (fillers, a repeat, a rewrite that adds a word, a three-word cut, a spoken path). `enhanced::tests::a_runaway_generation_stops_at_the_budget_and_falls_back` asserts the budget a request carries and the fallback after one attempt. `enhanced::tests::a_rewrite_that_drops_a_meaningful_word_inserts_the_draft` runs a dropped `not` through the whole pass. `dettivo-engine-llm`'s `a_generation_that_would_run_on_stops_at_max_tokens` holds the engine to the budget with the real Qwen3 1.7B model.

Bounds: the budget is an estimate, and on the 10 of 431 transcripts where the tokenizer exceeds one token per three characters it is that much tighter than 1.5 times the real tokens. The filler list is English with the German hesitation sounds. A dropped meaningful word in another language counts, but a dropped filler of another language is rejected as meaningful.
