# 0023. The Polish layers are ported verbatim from macOS and sit behind one provider interface with a trust gate

Status: Accepted 2026-09-04, amended 2026-09-05 (the raw layer's token rule) and 2026-09-06 by 0048 (Polish hides protected tokens; the numeric guard compares numbers), amended 2026-09-06 by 0049 (a redirect is never followed and a preset's model binds the request)

## What this gives you

Dictation gets its two other modes. `deterministic_polish` cleans a transcript with no model at all — fillers gone, casing and punctuation right, spoken paths and lists reassembled — so it is instant, private and identical on both ports. `enhanced` sends that result to a language model for a rewrite and keeps the deterministic text whenever the model is slow, absent or wrong, saying which of the three happened. Every rewrite runs against a model on this machine unless you have confirmed a remote endpoint once by name, and the confirmation is a line in your own `config.toml`.

## Situation

The macOS port carries the whole text layer in `Dettivo/Services/Polish/`: `LocalPolishFallback` (the deterministic rewrite), `PostProcessors` (787 lines of spoken-form repair — `@path` insertion, spoken slashes, business abbreviations, spoken lists, model identifiers), `PolishPolicyResolver` (which preset, style, transforms and post-processors a session runs under, resolved from the focused app), `PolishPromptBuilder` (the safety contract, the unspoofable transcript markers, the worked examples), `PolishOutputGuard` and `PolishNoOpGuard` (what a rewrite may not do), `ThinkingBlockStripper`, and `PolishRewriteEngine` (the attempt loop with its repair pass). Its test suites are the specification: `PostProcessorsTests`, `LocalPolishFallbackTests`, `PolishEvalRegressionTests` and `WarpHistoryPolishRegressionTests` are 90-odd exact input-output cases drawn from real dictations.

Two things could have gone wrong here. Rewriting those rules from their prose description would have produced a second dialect of Polish, and the masterplan (FR-M8) asks for parity that can be proven, not asserted. And the provider question — Ollama, an OpenAI-compatible endpoint, a future in-process llama.cpp engine — invites a design where the rewrite path knows about each one.

The privacy question is sharper than either. A dictation is often the most sensitive text a person types that day. macOS answers it with `PolishRemoteEndpointAllowlist` and `PolishPrivacyGate`: loopback is free, anything else needs one explicit confirmation stored by canonical URL.

## Decision

`crates/dettivo-language` carries the layers, ported case for case, and the macOS test suites are checked in as data under `crates/dettivo-language/tests/goldens/` (`polish_fallback.json`, `post_processors.json`, `at_prefix.json`, `warp_regressions.json`). A golden runner drives them; a difference from macOS fails the build with the case's name. The goldens are the parity claim, and they may only grow.

The crate is five modules. `raw` applies the replacement table, spoken punctuation and whitespace discipline, and moved here out of `dettivo-session` so one place owns text. `polish` is the deterministic pass with the transforms, the styles and the post-processor pipeline. `policy` resolves the effective policy in the macOS order — the user's app profile, the built-in app mapping, the app class, the global defaults — and freezes it with an eight-character SHA-256 hash of everything that went into it. `enhanced` runs the model attempt loop with the prompt profile, the output guard, the no-op guard and the repair pass. `provider` is one trait, `LlmProvider` (`name`, `model`, `probe`, `rewrite` with a timeout), with `ollama`, `openai_compatible`, a `local` stub that reports itself unavailable until the llama.cpp engine spec lands, and the QA mock behind `DETTIVO_MOCK_LLM=echo|fixture:<dir>`. `pipeline::run` is the one entry point the session and `polish.test` both call, so a sample and a dictation cannot disagree.

The trust gate lives in `provider::trust`: loopback hosts are allowed, every other endpoint is canonicalised (scheme and host lowercased, credentials, query and fragment dropped, a trailing `/v1` removed) and looked up in `[llm] trusted_endpoints`. An unconfirmed remote never receives a transcript; `llm.endpoints.trust` and `dettivo llm trust <url>` add it, and `llm.providers.list` says so with the command to run.

The raw layer protects tokens before it does anything else. A single left-to-right scan over characters (`raw::protect`) marks URLs (`scheme://` or `www.`), email addresses, paths and file names (a dot or a slash joining word characters with no whitespace), versions, dotted and `::` identifiers and backticked spans; opening brackets and quotes before a token and sentence punctuation after it stay outside the span. A replacement may cover a span whole, so a rule naming `index.ts` still applies, but never cut into one; spoken punctuation and whitespace discipline run over the text with each span swapped for a private-use placeholder and put back afterwards, so the sentence-end rule cannot fire on a dot inside a token and a span comes back byte for byte. The rule exists because every held-out polish-eval run failed its gate on `index.ts` becoming `index. Ts` before a model was judged, and because a mangled file name is exactly the damage FR-M2 forbids. `[dictation] protect_tokens` switches it; off restores every dot as a sentence end. The Polish pass and the guards did not change: the code preset still puts `@` before a path and a `?` inside a URL still reads as a question to `ensure_terminal_punctuation`, both macOS behaviours the goldens hold.

Three departures from macOS are deliberate.

The output guard gains a protected-token check. The macOS guard exempts technical text from its content checks, which is exactly the text where a dropped path costs the most, so a rewrite that loses a path, a backticked code span or a number the dictation carried is rejected here and the deterministic result goes in instead.

The provider layer's HTTP calls run on a thread of their own. The providers use a blocking client, the daemon's handlers run inside Tokio, and building a blocking client there panics; routing every call off the runtime keeps a probe from taking a client's connection down with it.

A rerun and an import stay on the raw layer. `transcripts.rerun` and `transcripts.import` answer `NOT_IMPLEMENTED` for any other mode, naming the layer they run, until the import spec carries the policy through them.

## Consequences

`dictation.start` accepts `raw`, `deterministic_polish` and `enhanced`, with the contract's `polish` an alias for the second; `system.capabilities.polish` and `system.capabilities.llm` say what this daemon serves. The completion `dictation.state`, the history item and `polish.test` all carry the mode, the policy hash and — when Enhanced inserted the deterministic result — a notice naming `provider_unavailable`, `guard_rejected` or `fallback_used` with a reason. Migration `0003-polish-policy` adds the policy hash column and the notice shares the column migration `0002-segments` added for the `silent` notice; rows written before them have neither.

`[polish]` and `[llm]` are the configuration, and `polish.rules.*`, `polish.apps.set` and `llm.endpoints.trust` write through the daemon's comment-preserving edit path (`dettivo_core::config::edit::set_structured` for the tables), so a file a person wrote by hand comes back with its comments.

The whole Enhanced pass is bounded by `[llm] timeout_ms` (8000 by default): when it runs out, the Polish text is inserted and the notice says `provider_unavailable`. The rewrite is not awaited afterwards.

The cost of verbatim porting is that the canonical-token table in `polish/post.rs` still names macOS paths (`SettingsView.swift`, `Dettivo/Services/Polish/PolishRewriteEngine.swift`). They are harmless on Linux and they keep the goldens honest; a later spec may retire them together with their cases.

`dettivo-session` now depends on `dettivo-language` and `dettivo-core`, and `dettivo-language` on `dettivo-core`: the mode step reads `[polish]` and `[llm]` straight from the schema rather than through a second vocabulary. `tools/xtask/src/edges.rs` records both edges.
