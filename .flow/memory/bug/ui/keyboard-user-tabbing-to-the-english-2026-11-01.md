---
title: Keyboard user tabbing to the English Models footer sees its note clipped — scrol
date: "2026-11-01"
track: bug
category: ui
module: qt/qml first-run OnboardingRoute / ModelsStep footer
tags: [qa, fn-80-a-fast-first-dictation-and-the-right, first-run]
problem_type: ui
symptoms: Tab to Back on the 5-row English Models step scrolls only until Back meets the window edge; footer note's last line half-clipped
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
---

## Problem
A keyboard user on the English first-run Models step tabs to the footer. Focus reaches Back and the page scrolls, but only until Back's bottom edge meets the window edge: the footer note's second line ("file.") stays half-clipped, and the page could still scroll further (the mouse wheel reaches a resting position about 40 px lower where the whole footer shows with margin).

## Steps to reproduce (cold)
1. Isolated profile, no speech model on disk, `config.toml` with `[dictation] language = "en"` and the Vulkan engine directory.
2. Start `dettivod` in that profile inside `scripts/qa/xvfb-session.sh` (Xvfb 1600x1000, openbox).
3. Launch `build/qt/apps/dettivo-app/dettivo-app` with `DETTIVO_QA_MODE=1 DETTIVO_QA_ALLOW_RELEASE=1 DETTIVO_E2E_STEP=models`; window 1280x820.
4. Wait 8 s, activate the window, press Tab 8 times (focus lands on Back), screenshot.

## Expected
The fix for QA-fn80-1 makes the footer "reachable by scrolling and by Tab"; a focused control's surroundings are readable when the page scrolls it into view ("Design is the bar").

## Actual
Back is focused and fully visible with its focus ring, flush with the window's bottom edge; the footer note's last line is cut in half. Continue is disabled at this stage so Back is the only footer stop. Scrolling with the wheel shows the full footer with margin and no overlap. Reproduced on two independent runs (pixel-identical).

## Evidence
- screenshot (Tab to Back): .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/rc-s6b-en-tab8.png
- screenshot (repro): .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/rc-s6b2-en-tab8.png
- screenshot (wheel scrolled, full footer): .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/rc-s6b-en-scrolled.png
- app log: .flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/rc-s6b-en-app.log
- url: dettivo-app first run, step `models`

## Traceability
- R-IDs: [R6]   scenario: S6b   driver_rung: xvfb root screenshot + xdotool keys   viewport: 1280x820 window on 1600x1000 Xvfb
