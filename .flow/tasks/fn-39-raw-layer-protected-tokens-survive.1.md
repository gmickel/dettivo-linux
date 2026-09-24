---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-39-raw-layer-protected-tokens-survive.1 Implement fn-39: protected tokens survive sentence casing and replacements stay out of URLs

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-39 done summary

The raw layer now keeps the tokens a person dictates whole. A new single-pass scanner, `crates/dettivo-language/src/raw/protect.rs`, marks URLs (`scheme://` and `www.`), email addresses, paths and file names (a dot or slash joining word characters with no whitespace), versions, dotted and `::` identifiers and backticked spans, with opening brackets and quotes before a token and sentence punctuation after it left outside the span. `raw::apply` gained a `protect_tokens` switch: replacements run over the plain text with a mask that lets a rule cover a span whole but never cut into it, and spoken punctuation plus whitespace discipline run with each span swapped for a private-use placeholder that is put back afterwards, so the sentence-end rule can no longer fire on the dot in `index.ts` and every span comes back byte for byte. `raw.rs` became `raw/{mod,protect,tests}.rs`, each well under the 500-line bound.

R1 is pinned by `raw/protect.rs` unit tests per token class (file name, path, `~/` and `./` paths, URL with a query, `www.` host, email, `v1.2.3` and `1.2`, `foo.bar()`, `std::io::Error`, a backticked span, a non-ASCII file name), by `raw/tests.rs` running each class through the whole raw pass with a spoken sentence end after it, by the sentence-really-ends cases (`open index.ts. Then run it`, `see example.com.`) and by a `pipeline.rs` test that runs each class through `raw` and `deterministic_polish` and fails naming the token whose bytes changed. R2 is pinned by a test with the rule `example -> sample` against a sentence carrying the word plainly, inside `https://example.com/example` and inside `gordon@example.org` (only the plain word changes), plus a rule `ts` against `index.ts is ts`, an abbreviation rule `e.g.` applying on plain text, and a rule naming `index.ts` whole still applying. `protect_tokens = false` is tested to restore the old `Open index. Ts now` behaviour.

R4 shipped as the `[dictation] protect_tokens` key in `schema.rs` (default `true`), the default TOML, `docs/config.md` and the `config.print_default` fixture, and the `polish.test` fixture now dictates `src/app/index.ts`, `https://example.com/docs` and `v1.2.3` through the code preset (the fixture layout allows one success fixture per method, so the example replaced the `um update readme` input rather than sitting beside it). The contract replay passes: 70 passed, 0 failed, 25 skipped, 19 pending. R5 shipped in `docs/dictation.md` (the modes paragraph and the configuration table), `docs/polish.md` (the opening and a second `dettivo polish test` example with a file name and a URL) and ADR 0023, which now records the token rule and why it exists instead of a new record.

R3 is partial by design of the branch layout. The fn-30 polish-eval harness, its sample set (`crates/dettivo-qa/fixtures/polish-eval/sample.jsonl`), the dictionary eval (`crates/dettivo-language/tests/dictionary_eval.rs`) and `docs/reports/polish-eval/` live on PR #29 and are not on `main`, so there were no fixtures on this branch to add rows to. The token classes are pinned here in the raw and pipeline tests instead. Once fn-30 merges, the remaining R3 work is: add one row per token class to the sample set and the dictionary eval, regenerate the golden, and rerun the held-out report on this machine to confirm the guard no longer rejects `index.ts`-style rows. The eval fixtures do not exist on this branch, so no golden was touched.

Deliberately left out: the Polish pass and the guards are unchanged (the code preset still puts `@` before a path and a `?` inside a URL still makes `ensure_terminal_punctuation` pick a question mark, both macOS goldens); no dictionary corrector; no change to the LLM prompts. The pinned macOS contract document `docs/api/dettivo-ipc-v1.md` still shows the old `um update readme` sample, as its hash is pinned.

Verified with `cargo test -p dettivo-language`, `cargo test -p dettivo-proto`, `cargo run -q -p dettivo-qa -- contract` and the full `make build test lint` gate.
## Evidence
- Commits: a4e8f2fe3b1ed3e27f194b4b6e81a47e0cdd2fe3, d0229eb659c0a6311739298edabdd0a73fc1f960, 0fd228c768b6650e355c401acd8b2a14d606919c, f8ff4589107e7202103dded489b43d71404eae0e, 21881ab4cfc89f248605b119e0a06c2173068746
- Tests: cargo test -p dettivo-language, cargo test -p dettivo-proto, cargo run -q -p dettivo-qa -- contract, make build test lint
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
