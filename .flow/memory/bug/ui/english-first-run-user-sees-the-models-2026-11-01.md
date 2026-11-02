---
title: English first-run user sees the Models step overflow into its footer — fifth rec
date: "2026-11-01"
track: bug
category: ui
module: qt/apps first-run Models step (FirstRunModels rows)
tags: [qa, fn-80-a-fast-first-dictation-and-the-right, first-run]
problem_type: ui
symptoms: With language en the Parakeet v2 recommendation adds a fifth speech row and the Enhanced 'Use Ollama instead' row draws over the footer note and Back/Continue
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/cli-user-reading-config-keys-cant-learn-2026-11-01, bug/ui/cli-users-comma-less-last-vocabulary-2026-11-01, bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-keeps-seeing-a-vocabulary-2026-11-01, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
A new English-speaking user opening first run on a GPU machine sees the Models step's Enhanced rows run into the footer: "Use Ollama instead" and its line overlap the "Writes [speech] and [llm] to config.toml" note and the Back / Continue buttons.

## Steps to reproduce (cold)
1. Isolated profile, no speech model on disk, `config.toml` with `[dictation] language = "en"` and the Vulkan engine directory.
2. Start `dettivod` in that profile inside `scripts/qa/xvfb-session.sh` (Xvfb 1600x1000, openbox).
3. Launch `build/qt/apps/dettivo-app/dettivo-app` with `DETTIVO_QA_MODE=1 DETTIVO_QA_ALLOW_RELEASE=1 DETTIVO_E2E_STEP=models`.
4. Wait 8 s, screenshot the root window.

## Expected
R6: "First run preselects the recommended model for the user's chosen language" - the step stays readable: every row and the footer visible without overlap (docs/app.md design size 1280x820, "Design is the bar").

## Actual
Parakeet v2 is correctly preselected and marked RECOMMENDED with "word error 4.1 %". Because the recommendation adds a fifth speech row (small.en, large-v3-turbo, Parakeet v2, Parakeet v3, Parakeet Ultra), the column grows past the window: the "Use Ollama instead" row is cut through by the footer, its text drawn over "Writes [speech] and [llm]..." and next to the Back / Continue buttons. German (4 rows) lays out cleanly. Reproduced twice.

## Evidence
- screenshot: .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/r6b-en-models.png
- screenshot (repro): .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/r6b2-en-models.png
- clean comparison (de, 4 rows): .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/r6a-de-models.png
- app log: .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/r6b-en-app.log
- url: dettivo-app first run, step `models`

## Traceability
- R-IDs: [R6]   scenario: S6b   driver_rung: xvfb root screenshot   viewport: 1280x820 window on 1600x1000 Xvfb
