# Beauty iteration against the original Studio artboards

Home and first run now align more closely with the authored Studio layouts. History and Meetings retain their readable content at short and narrow window sizes. This follow-up to the [live desktop QA pass](2026-09-08-live-desktop.md) implements Gordon's request to continue iterating on PR #53.

This report covers fn-54 tasks .2 through .6. The final QA receipt names the tested implementation head. Original artboards, approved app-render images and visual thresholds are unchanged. Gordon's fn-38 R6 approval remains his checkpoint.

## Changes and evidence

Evidence paths below are relative to `.flow/tmp/beauty-20260908/`. Captures use isolated fixtures and include original-source comparisons; they do not replace release or human approval.

| Area | Confirmed issue and correction | Evidence |
|---|---|---|
| Home | Today inherited a row inset that disagreed with its heading. It now aligns with the heading, while other consumers retain their default inset. Engine progress now has separate tracks. | `home-first-run/before-home.png`, `after-home.png`, red/green Home tests |
| First run | Branding sat eight pixels too low and model state/size columns had different alignments. Header height and columns now follow the authored structure. | `home-first-run/before-{keys,models,try}.png`, corresponding `after-*.png`, first-run regressions |
| Live transcript | Initial positioning clipped the first line, and provisional text growth could leave the tail offscreen. Settled layout and gesture-aware following correct both. | `transcripts-meetings/before-live-black-gold.png`, `after-live-black-gold.png`, `red-scrollbar.log`, `green-interactions.log` |
| Analysis | Long text painted over the footer facts in a short window. A focus-revealing viewport keeps text and actions above those facts. | `transcripts-meetings/before-short-black-gold.png`, `after-short-black-gold.png`, matching Latte captures |
| Panels and rows | History/meeting content panels had heavier borders and filled interiors than the artboards. Their hairline/transparent treatment now matches the reference intent; controls retain their focus styling. Meeting columns center on multiline rows. | History and meeting before/after captures in `transcripts-meetings/`; original History border RGB 44,42,36 is reproduced |
| Model controls | Normal model labels truncated despite spare row width. Selectors are wider and the idle field names its seconds unit; keys and integer writes are unchanged. | `compact-controls/before-settings-normal.png`, `after-settings-normal.png`, label-fit regression |
| New meeting | At 410-pixel window height, the controls overlapped the configuration footer. The rail now scrolls above it. Source and fact controls have keyboard activation and themed focus. | `compact-controls/before-meetings-short.png`, `after-meetings-short-focus.png`, matching light captures |
| Narrow Meetings list | The title collided with Search and the title column became -123 pixels wide. The compact header stacks and the table preserves its column widths behind a keyboard-accessible horizontal viewport. | `meetings-list/before/`, `meetings-list/verified/`; normal/minimum/short, populated/empty, dark/light |
| Engine menu | Live keyboard QA exposed underlying page text through a 3%-opaque menu background, and choices truncated to indistinguishable names. The menu now has an opaque background, fits its item labels and stays within window bounds. | `menu-red.log`, `menu-bounds.log`; live before/after paths below |

The normal Meetings views remained pixel-identical across the compact-rail and compact-list changes. Four final normal-size comparisons, populated and empty on Black Gold and Catppuccin Latte, returned ImageMagick AE=0. The minimum-width version was inspected separately after the search field was aligned to the header's left edge.

## Visual review

These unmodified evidence captures are separate from approved baselines. Home, analysis and narrow-list examples use fixture data; the menu examples use the isolated live daemon with seeded records.

| Comparison | Before | After |
|---|---|---|
| Home alignment | [Before](beauty-20260908/home-before.png) | [After](beauty-20260908/home-after.png) |
| Long analysis in a short window | [Before](beauty-20260908/analysis-before.png) | [After](beauty-20260908/analysis-after.png) |
| Minimum-width Meetings list | [Before](beauty-20260908/meetings-before.png) | [After](beauty-20260908/meetings-after.png) |
| Live engine menu | [Before](beauty-20260908/menu-before.png) | [After](beauty-20260908/menu-after.png) |

The narrow-list worker reached its usage limit after writing the implementation. The conductor inspected the diff and reran its build, all 74 Qt tests and lints before completing task .5. An interrupted result was not accepted as proof of completion.

## Integrated verification

| Check | Result |
|---|---|
| GUI pack at `0135f411` | Passed 84 routes, zero text findings, accessible naming 1.000 across inspected controls. Two installed Omarchy steps skipped. Report `gui-final/run-1788899915-1072850/pack-gui/gui-pack.json`. |
| Visual matrix at `0135f411` | 590 entries, 393 passed, 107 differences, 90 awaiting first approval; zero style findings. Report `visual-final/visual-report.json`. |
| Live minimum-window keyboard walkthrough | At 969x410, Tab focused the table, End revealed its final columns with the focus ring fixed to the viewport, and Space toggled the System audio checkbox. |
| Subsequent shared-menu repair | Three regressions passed, including extreme content bounds and keyboard activation. All 74 Qt tests and QML/token/accessibility lints passed. A fresh live menu capture shows complete model labels on an opaque background; keyboard selection persisted `speech.meeting_model = tiny`. |
| Full repository gate | Passed. `GATE_beauty-delivery_EXIT=0 50e7ca49`, log `/tmp/dtv-gate-beauty-delivery.log`. All Rust tests, all 74 Qt tests and required lints passed. |

Direct live evidence is under `.flow/tmp/qa-live-20260908/beauty-final/` and `beauty-menu/`. The former contains `meetings-minimum.png`, `table-end-visible.png`, `source-toggled.png` and two reproductions of the translucent menu. The latter contains `menu-fixed.png`, `menu-selected.png` and `selected-config.json`. These use an isolated live daemon. The full GUI/matrix runs preceded the menu repair; its affected behavior was rechecked through focused tests and a fresh live session.

Of the 107 visual failures, 103 compare against older approved app renders and four compare directly against Black Gold artboard crops. The four direct failures are Today at 1x/2x and Models selections at 1x/2x. Today now aligns with its heading but still has theme-font and runtime-metadata differences; the selections intentionally allocate more width to readable real labels and show seconds. These crops remain below the structural threshold and require human review. This report makes no claim that the artboard or approved-render gate is green.

Each completed implementation task passed the Qt build, all 74 CTest checks, QML lint/format, token lint and accessibility-name lint. New tests reproduce column overlap, clipped transcript position, interrupted follow behavior, long analysis overlap, label truncation and keyboard activation/focus. The task receipts name their exact commands and artifacts.

The first integrated gate found the Home test file three lines over the 300-line limit. Its engine fixture was extracted without changing the assertions; the Home tests, file-length check, QML lint and full gate then passed. Packaging still reports shellcheck unavailable, so that check remains skipped.

## Intentional differences and remaining approval

- Typography follows the active Omarchy theme. The fixture's 12-pixel body and 20-pixel title differ from the artboards' 13/24-pixel typography; this pass preserves that contract.
- Dates, model/provider names, bindings, waveform samples and enabled actions follow their actual source. Illustrative artboard facts are not substituted into live state. Fake-data layout captures are identified separately from live-daemon evidence.
- The existing app-render baselines predate these layout corrections. Their differences remain visible in the matrix; the artboard score is separate from the approved-render gate.
- The memory-budget recalibration is captured in fn-55 and remains unimplemented. This beauty work changes neither the active memory limit nor its historical failure.
- Installed shell/socket coverage, Gordon's final design walkthrough and the pending fn-38 work remain separate. No approval was authored in Gordon's name, and the held cleanup/GPU work was not started.

The implementation decisions are in [ADR 0056](../../adr/0056-live-desktop-actions-and-evidence.md).
