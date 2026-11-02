---
title: Agent guide exit-code table omits exit 6 (NoSpeech)
date: "2026-11-01"
track: bug
category: ui
module: docs/guides/agents.md
tags: [qa, fn-85-delivery-that-leaves-the-desktop-as-it, docs]
problem_type: ui
symptoms: docs/guides/agents.md Exit codes lists 0-5; dictation stop on a silent take exits 6
root_cause: (observed via live QA — unconfirmed)
resolution_type: fix
related_to: [bug/ui/cli-user-reading-config-keys-cant-learn-2026-11-01, bug/ui/cli-users-comma-less-last-vocabulary-2026-11-01, bug/ui/dictation-docs-omit-the-silence-peak-2026-11-01, bug/ui/dictation-stop-on-a-gated-take-answers-2026-11-01, bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10, bug/ui/settings-user-keeps-seeing-a-vocabulary-2026-11-01, bug/ui/settings-user-sees-the-file-changed-on-2026-11-01]
---

## Problem
An agent author reading the agent guide's exit-code table does not learn about exit 6. `dettivo dictation stop` and `stop --print` on a silent take now exit 6 (`NoSpeech`), and `docs/daemon.md` lists it, but `docs/guides/agents.md` "Exit codes and errors" still stops at 5.

## Steps to reproduce (cold)
1. Private profile with `DETTIVO_MOCK_MIC=silence.wav` (2 s of near-silence), `DETTIVO_MOCK_INSERT=1`.
2. `dettivo dictation start`; after 1 s `dettivo dictation stop --print`: exit 6, `dettivo: no speech was heard (CONFLICT)`.
3. Open `docs/guides/agents.md` section "Exit codes and errors".

## Expected
R4: "exits 6 (`NoSpeech`, documented in the CLI exit map) when no speech was heard". The agent guide is the page README points agent authors to for exit codes ("The `dettivo` command tree, `--json` and exit codes").

## Actual
`docs/daemon.md` lines 75-85 list 6. `docs/guides/agents.md` lines 176-185 list 0 to 5 only, while the same file's command tree was updated for `stop --print`.

## Evidence
- `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/runs/r4-silence-p2/print-rc.txt` (6), `print-stderr.txt`, `run.log`
- `.flow/tmp/qa-fn-85-delivery-that-leaves-the-desktop-as-it/runs/r4-silence-p3/print-rc.txt` (6)
- `docs/guides/agents.md:176-185`, `docs/daemon.md:75-85`

## Traceability
- R-IDs: [R4, R6]   scenario: S4b-print-silent   driver_rung: CLI   viewport: n/a
