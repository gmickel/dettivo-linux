---
title: Dictation docs omit the silence_peak_threshold retirement (config.md only)
date: "2026-11-01"
track: bug
category: ui
module: docs/dictation.md
tags: [qa, fn-79-clean-dictation-takes-a-real-speech, dictation]
problem_type: ui
symptoms: docs/dictation.md never names the retired silence_peak_threshold key; R8 asks for both docs
root_cause: (observed via live QA — unconfirmed)
resolution_type: documentation
related_to: [bug/ui/doctor-tells-a-user-to-download-a-2026-11-01, bug/ui/live-qa-exposes-inactive-home-action-2026-09-08, bug/ui/meeting-export-option-text-extends-2026-09-10]
---

## Problem
A user reading `docs/dictation.md` for the speech gate finds `[dictation] silence_dbfs` in the configuration table but nothing saying it replaces `silence_peak_threshold` or what happens to a config that still sets the old key. The migration note lives only in `docs/config.md` (and the changelog).

## Steps to reproduce (cold)
1. `grep -n -i peak docs/dictation.md` on branch head 1481d34: the only hit is the `audio.level` payload row.
2. `grep -n silence_peak_threshold docs/config.md CHANGELOG.md`: config.md line 152 and CHANGELOG.md line 15 carry the migration.

## Expected
R8: "Any config migration or deprecation is documented in `docs/config.md` and `docs/dictation.md`, and a changelog entry is added."

## Actual
The runtime behaviour is right (a config carrying `silence_peak_threshold = 0.9` loads, doctor reports `config valid`, the value is ignored and JFK dictates in full, two passes), but `docs/dictation.md` does not mention the retired key.

## Evidence
- runs/p1-r8-legacy-peak-jfk/, runs/p2-r8-legacy-peak-jfk/ (config.toml, doctor.txt, inserted.txt, events.jsonl)
- docs/dictation.md (no `silence_peak_threshold`), docs/config.md:152, CHANGELOG.md:15

## Traceability
- R-IDs: [R8]   scenario: S12   driver_rung: CLI over the isolated daemon socket
