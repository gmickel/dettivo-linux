---
title: Settings user keeps seeing a vocabulary refusal after the entry is gone
date: "2026-11-01"
track: bug
category: ui
module: qt/qml/Dettivo/app/settings/VocabularySection.qml
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, settings]
problem_type: ui
symptoms: refusal naming Omarchy persists after a hand edit removed Omarchy
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
A refused vocabulary save leaves its red error on the Settings / Vocabulary page after a hand edit of config.toml removes the offending entry and the page refreshes.

## Steps to reproduce (cold)
1. Settings / Vocabulary open (Xvfb rig). Enter 17 spellings in "Sounds like for Omarchy", press Enter.
2. The error `dictation.vocabulary: "Omarchy" has 17 sounds_like spellings; a term takes at most 16` shows (correct).
3. Hand-edit config.toml to `vocabulary = ["Dettivo", "Clef"]`; the page refreshes its rows.

## Expected
The error describes the current state; once Omarchy is gone it should clear.

## Actual
Rows refresh but the error naming "Omarchy" stays; reproduced twice. Error retention is the shared settings model's behaviour, likely pre-existing.

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/c-gui/screenshot-7g-hand-edit.png
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/c-gui-repro/tree-r2-after-hand-edit.json

## Traceability
- R-IDs: [R2, R5]   scenario: S7   driver_rung: AT-SPI under Xvfb   viewport: 1600x1000
