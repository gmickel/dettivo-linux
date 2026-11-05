---
title: Settings user sees 'The file changed on disk' after the page's own save
date: "2026-11-01"
track: bug
category: ui
module: qt/qml/Dettivo/app/settings (SettingsPage hand-edit notice)
tags: [qa, fn-83-vocabulary-entries-with-spoken-aliases, settings]
problem_type: ui
symptoms: own save shows the hand-edit notice in place of the subtitle
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10]
---

## Problem
After the Settings / Vocabulary page saves a change itself (Add term, or a Sounds like edit), the page subtitle is replaced by "The file changed on disk; 1 value(s) refreshed." although nobody edited the file by hand.

## Steps to reproduce (cold)
1. Isolated daemon + `dettivo-app` with `DETTIVO_E2E_OPEN=settings DETTIVO_E2E_ROUTE=vocabulary` under scripts/qa/xvfb-session.sh.
2. Type `Omarchy` in "Vocabulary term", press Add term (or edit "Sounds like for Dettivo" and press Enter).
3. Read the label under the page title.

## Expected
The hand-edit notice is for a file changed outside the app (docs/qa.md settings_roundtrip: "a hand edit of the file refreshes the open route with the notice").

## Actual
The notice appears after the page's own write; reproduced three times (aliases edit twice, plain Add term once). The plain Add term path predates fn-83, so the cause is pre-existing.

## Evidence
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/c-gui/screenshot-7b-dettivo-aliases.png
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/c-gui-repro/tree-r1-after-own-save.json
- .flow/tmp/qa-fn-83-vocabulary-entries-with-spoken-aliases/c-gui-preexisting/tree-r3-after-add.json

## Traceability
- R-IDs: [R5]   scenario: S7   driver_rung: AT-SPI under Xvfb   viewport: 1600x1000
