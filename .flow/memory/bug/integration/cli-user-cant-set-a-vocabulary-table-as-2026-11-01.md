---
title: CLI user can't set a vocabulary table as JSON — refusal says a list of strings i
date: "2026-11-01"
track: bug
category: integration
module: dettivo config set / crates/dettivo-core/src/config/edit.rs
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, cli]
problem_type: integration
symptoms: config set with a JSON object entry answers "expected a list of strings"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/integration/at-spi-history-input-loses-a-leading-2026-09-10]
---

## Problem
A CLI user typing a vocabulary with a JSON object entry into `dettivo config set dictation.vocabulary` gets a refusal that says a list of strings is expected, though the list now takes `{ term, sounds_like }` tables.

## Steps to reproduce (cold)
1. Isolated profile daemon (fn-83 QA rig, Xvfb :183), any `[dictation] vocabulary`.
2. `dettivo config set dictation.vocabulary '["Dettivo", {"term": "Hyprland", "sounds_like": ["hyper land"]}]'`
3. Observe stderr and exit code.

## Expected
R1/R5: "The CLI accepts both forms." The file-syntax form (`{ term = "Clef", sounds_like = [...] }`) is accepted; a refused value should say what the list may hold.

## Actual
`dettivo: dictation.vocabulary: expected a list of strings, got "[...]" (INVALID_PARAMS)`, exit 4, reproduced twice. The same JSON value over the contract (`dettivo call config.set '{"key":"dictation.vocabulary","value":[...]}'`) is accepted and written as TOML tables. `config.set` with `sounds_like: [1]` also answers "expected a list of strings". The message predates the change; with table entries it now misleads.

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/a-config/run.log (S2b)
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/a2-config/run.log (S2b retry + JSON API)

## Traceability
- R-IDs: [R1, R5]   scenario: S2   driver_rung: CLI against isolated daemon
