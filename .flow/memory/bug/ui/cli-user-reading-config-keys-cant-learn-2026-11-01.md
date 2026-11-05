---
title: CLI user reading config keys can't learn the vocabulary table form
date: "2026-11-01"
track: bug
category: ui
module: dettivo config keys / config key descriptions
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, cli]
problem_type: ui
symptoms: dictation.vocabulary described as Words ... only
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-keeps-seeing-a-vocabulary-2026-11-01, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
`dettivo config keys` still describes `dictation.vocabulary` as "list ... Words the engine is primed with: names, products, jargon." and never mentions the `{ term, sounds_like }` table form or the limits.

## Steps to reproduce (cold)
1. `dettivo config keys | grep '^dictation.vocabulary'` (no daemon needed).

## Expected
R7 updates docs/dictation.md, docs/config.md and the delta list (done); the CLI's own key reference is where a terminal user looks.

## Actual
`dictation.vocabulary  list  default []  Words the engine is primed with: names, products, jargon.` (twice).

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/config-keys.log

## Traceability
- R-IDs: [R5, R7]   scenario: S1   driver_rung: CLI
