---
title: Meeting export option text extends beyond the sheet border
date: "2026-09-10"
track: bug
category: ui
module: qt/qml/Dettivo/app/meetings/MeetingExportSheet.qml
tags: [qa, fn-53, meeting-export]
problem_type: ui
symptoms: Raw-text checkbox is 393 pixels wide inside a 352-pixel export content column.
root_cause: Observed width overflow; the checkbox has no width constraint.
resolution_type: fix
related_to: [bug/ui/live-qa-exposes-inactive-home-action-2026-09-08]
---

# Meeting export option extends beyond its sheet

P2, confirmed by two fresh export-sheet observations during the final live notes probe. Export completed correctly in both cases. The raw-text option is 393 logical pixels wide inside a 352-pixel content column, and its text paints beyond the sheet's right border. This is a separate cosmetic finding; this pass did not change that control or approve its appearance.

## Reproduce

Run the retained `notes-export-probe-contract` under `scripts/qa/xvfb-session.sh` with `DETTIVO_BUILD_PROFILE=release`. Its source and binary hashes are in `/tmp/dettivo-combined-qa-20260910-Nyxanz/notes-probe-contract-provenance.json`. The probe uses a fresh seeded profile and the existing AT-SPI driver, opens Roadmap review, selects Notes, then opens Export and selects JSON. The app is 1280 by 820 at DPR 1 on the builtin-dark theme.

## Expected and observed

The repository requires every surface to match the approved design baselines. The option label should fit inside the sheet. The accessibility trees record the content column as `[724, 299, 352, 417]` and the checkbox as `[724, 515, 393, 28]`. Both screenshots show the label extending outside the border. No functional export failure was observed.

## Evidence

Evidence base `/tmp/dettivo-combined-qa-20260910-Nyxanz/notes-contract/run-1789064967-1877975/combined_notes_exports.atspi/` contains:

- `screenshot-export-nonempty.png` and `tree-export-nonempty.json`.
- `screenshot-export-whitespace.png` and `tree-export-whitespace.json`.
- `exports/meeting-nonempty.json` and `exports/meeting-whitespace.json`, which preserve the intended notes semantics.
- `app-journal.jsonl` and `receipt.json`.

The application binary is identified in `/tmp/fn52-input-complete-binaries.json`. The latest source head at observation was `258cb001a97ab4532863132f5c8195319f52d6ba`, whose only delta from the tested code head `f324e285a57e02142d6f3d04d486438b15d48880` is a Flow task receipt.

## Disposition

Observed cosmetic defect, open. The bound label has no width constraint in `qt/qml/Dettivo/app/meetings/MeetingExportSheet.qml`; no repair or design approval is included. This supplemental visual observation has no new cleanup R-ID and does not replace Gordon's fn-38 human checkpoint.
