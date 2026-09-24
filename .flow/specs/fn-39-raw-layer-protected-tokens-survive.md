# Raw layer: protected tokens survive sentence casing and replacements stay out of URLs

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> fn-30 worker report (2026-09-05): "All four held-out runs fail the quality gates for both models with identical figures: the raw layer turns `index.ts` into `index. Ts` (sentence-end capitalisation inside tokens), so protected tokens are mangled before any model sees them and the guard rejects. Pre-existing raw-layer defect ... Also seen: `[dictation] replacements` applies inside URLs."
> masterplan FR-M2: the raw layer is deterministic and never damages what the user said; file names, identifiers and addresses pass through untouched.
> masterplan FR-M9: the held-out gates must pass on Linux with the same figures as macOS for the shared rows.

## Goal & Context

<!-- Goal & Context: 20% [user], 60% [paraphrase], 20% [inferred] -->

The deterministic raw layer in `dettivo-language` treats a dot followed by a letter as a sentence end, so a dictated `index.ts`, `v1.2`, `example.com` or `foo.bar()` comes out with a space and a capital after the dot, and `[dictation] replacements` rewrite text inside URLs and email addresses. Both mangle exactly the tokens the polish guards protect, which is why every held-out polish-eval run in fn-30 fails its quality gate before a model is judged. This spec makes the raw layer token-safe: a dot with no whitespace on either side inside a token stays a token, casing and spacing rules apply only at real sentence boundaries, and replacements skip URL, email, path and identifier spans. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- `crates/dettivo-language/src/raw/` gains a token scanner that marks protected spans before the sentence pass: URLs (scheme or `www.`), email addresses, file names and paths (a dot or slash joining word characters with no space), version strings (`v1.2.3`, `1.2`), identifiers with dots or `::`, and code-like spans in backticks. The sentence-end rule (space, capital) fires only when the dot is followed by whitespace or the end of text; the same guard covers `?` and `!` inside tokens. [paraphrase]
- `[dictation] replacements` and the auto-capitalisation pass consult the protected spans and leave them byte-identical; the spans are re-inserted after every transform. [paraphrase]
- The polish guards (fn-16) already compare protected tokens before and after a rewrite; with the raw layer fixed the guard's rejections on the held-out sets disappear for those rows. [inferred]
- The dictionary eval and the polish-eval sample set from fn-30 gain rows for each token class so the regression stays covered. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- No new daemon methods. `polish.test` in `raw` and `deterministic_polish` modes returns protected tokens unchanged; the fixtures under `crates/dettivo-proto/fixtures/polish/` gain an example with a file name and a URL. [paraphrase]
- Config: `[dictation] protect_tokens = true` (off restores the old behaviour for someone who wants every dot to end a sentence). [inferred]

## Edge Cases & Constraints

- A sentence that really ends with a file name ("open index.ts. Then run it.") keeps the second dot as the sentence end because whitespace follows it. [paraphrase]
- Trailing punctuation after a URL ("see example.com.") stays outside the protected span. [paraphrase]
- Replacements whose source itself contains a dot (an abbreviation rule) still apply to plain text, never inside a protected span. [inferred]
- Non-ASCII letters around a dot follow the same rule. [inferred]

## Acceptance Criteria

- **R1:** The raw layer's unit tests cover file names, paths, URLs, emails, versions, dotted identifiers, backtick spans and the sentence-really-ends case, and each protected token comes back byte-identical through `raw` and `deterministic_polish`. Errors: a case that changes bytes fails naming the token. [paraphrase]
- **R2:** `[dictation] replacements` never rewrites inside a protected span (test with a rule whose source appears inside a URL and outside it). Errors: as stated. [paraphrase]
- **R3:** The fn-30 sample set and the dictionary eval gain rows per token class, `dettivo-qa polish-eval` on the sample set passes its golden, and the held-out report rerun on this machine no longer rejects rows for `index.ts`-style tokens (the report is regenerated under `docs/reports/polish-eval/`). Errors: the guard still rejecting those rows fails the run. [paraphrase]
- **R4:** The contract fixtures for `polish.test` carry the protected-token example and the replay passes; the `[dictation] protect_tokens` key is in the schema, the default TOML and `config print-default`. Errors: as stated. [inferred]
- **R5:** `docs/dictation.md` and `docs/polish.md` describe the protected spans and the key; the ADR for the polish layers records the token rule. Errors: docs check fails. [paraphrase]

## Boundaries

- No dictionary corrector (the R4 parity gap of fn-30 stays open). [paraphrase]
- No change to the LLM prompts or guards beyond what the fixed input makes unnecessary. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- Every polish gate on Linux fails on this defect before a model is judged, and a dictated file name that comes out mangled is the kind of damage FR-M2 forbids. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** the held-out gates become meaningful once the raw layer stops damaging protected tokens. [strategy:Local engines and performance]
- **Complete speech workflows, proven by drives:** the token classes are pinned by tests and the eval rows. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
