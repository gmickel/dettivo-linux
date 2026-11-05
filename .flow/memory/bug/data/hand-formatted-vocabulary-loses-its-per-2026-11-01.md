---
title: Hand-formatted vocabulary loses its per-entry comments and layout on config set
date: "2026-11-01"
track: bug
category: data
module: crates/dettivo-core/src/config/edit.rs
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, cli]
problem_type: data
symptoms: "multi-line array collapsed to one line, item comments dropped"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
`dettivo config set dictation.vocabulary` (and Settings saves) rewrite a hand-formatted multi-line vocabulary array as one inline line and drop the comments written beside each entry.

## Steps to reproduce (cold)
1. Profile config.toml with
   ```
   [dictation]
   # My vocabulary; this comment must survive every write
   vocabulary = [
     "Dettivo", # plain string
     { term = "Clef", sounds_like = ["clay", "cleff"] }, # table form
   ]
   ```
2. `dettivo config set dictation.vocabulary '["Dettivo", { term = "Clef", sounds_like = ["clay"] }]'`
3. Read the file.

## Expected
R1: "round-trips through `config set` keep comments and form."

## Actual
The comment above the key survives and each entry keeps its string/table form, but the array becomes a single line and `# plain string` / `# table form` are gone. The base build already replaced the array wholesale for plain-string lists, so this predates fn-83 (inferred from the base `coerce` code, not run on base).

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/a-config/s1-initial.toml vs s2a-after-file-syntax.toml
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/a2-config/after-json-api.toml

## Traceability
- R-IDs: [R1]   scenario: S2   driver_rung: CLI against isolated daemon
