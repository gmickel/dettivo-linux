---
title: CLI user's comma-less last vocabulary entry reshapes the array — closing bracket
date: "2026-11-01"
track: bug
category: ui
module: crates/dettivo-core/src/config/file_syntax.rs
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, config-set]
problem_type: ui
symptoms: "config set on a multi-line array whose last entry has no trailing comma leaves '\"Omarchy\" # c' then a ',' line and '\"SapienXT\"]'"
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/cli-user-reading-config-keys-cant-learn-2026-11-01, bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-keeps-seeing-a-vocabulary-2026-11-01, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
A CLI user whose multi-line vocabulary array ends with an entry that has no trailing comma gets a reshaped array after `dettivo config set dictation.vocabulary`: the closing `]` moves onto the last entry's line, and an appended entry is preceded by a line holding only `,`. Comments survive and the file stays valid; only the layout changes.

## Steps to reproduce (cold)
1. Isolated profile (own XDG dirs, own daemon socket) with config.toml:
   ```
   [dictation]
   # keep me (above the key)
   vocabulary = [
     "Dettivo", # item comment Dettivo
     { term = "Clef", sounds_like = ["clay"] }, # item comment Clef
     "Omarchy" # last entry, no comma, comment kept
   ]
   ```
2. `dettivo config set dictation.vocabulary '["Dettivo", { term = "Clef", sounds_like = ["clay", "cleff"] }, "Omarchy", "SapienXT"]'`
3. Read the file.

## Expected
R1: round-trips through `config set` keep comments and form; the QA-2 fix states multi-line arrays keep their layout.

## Actual
```
vocabulary = [
  "Dettivo", # item comment Dettivo
  { term = "Clef", sounds_like = ["clay", "cleff"] }, # item comment Clef
  "Omarchy" # last entry, no comma, comment kept
,
  "SapienXT"]
```
Replacing the tail instead (`'["Dettivo", {"term": "Hyprland", "sounds_like": ["hyper land"]}]'`) gives `  { term = "Hyprland", sounds_like = ["hyper land"] }]`. With a trailing comma on the last entry (the a2-config and Settings g-gui profiles) the layout is kept exactly, `]` on its own line. Reproduced in two runs with identical output.

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/recheck/run1/f-cli/f0-initial.toml
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/recheck/run1/f-cli/f2-after-set-file-syntax.toml
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/recheck/run1/f-cli/f1-after-json-cli.toml
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/recheck/run2/f-cli/run.log

## Traceability
- R-IDs: [R1]   scenario: S2   driver_rung: CLI against isolated daemon   viewport: n/a
