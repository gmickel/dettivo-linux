---
satisfies: [R1, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.3 Polish transcript and meeting layouts against Studio

## Description
Continue Gordon's requested beauty iteration on PR #53. Compare the existing full-window renders of History, Meetings list, live meeting and meeting detail against original Studio PNGs and authored HTML. Fix reproduced layout defects and unjustified visual mismatches while preserving live behavior and active theme typography.

**Touches:** qt/qml/Dettivo/app/history/DetailBlock.qml, qt/qml/Dettivo/app/history/AudioStrip.qml, qt/qml/Dettivo/app/history/HistoryDetail.qml, qt/qml/Dettivo/app/meetings/LiveTranscript.qml, qt/qml/Dettivo/app/meetings/TranscriptRow.qml, qt/qml/Dettivo/app/meetings/AnalysisRail.qml, qt/qml/Dettivo/app/meetings/AnalysisCard.qml, qt/qml/Dettivo/app/meetings/AnalysisBlock.qml, qt/qml/Dettivo/app/meetings/NotesEditor.qml, qt/qml/Dettivo/app/meetings/RailFactRow.qml, qt/qml/Dettivo/app/meetings/MeetingRow.qml, qt/qml/Dettivo/tests/qml/tst_app_history.qml, qt/qml/Dettivo/tests/qml/tst_app_meetings.qml, qt/qml/Dettivo/tests/qml/tst_meeting_layout.qml

Do not change Theme tokens, artboards, approved renders, visual thresholds or fn-38 approval/state. Inspect fn-38 code only as reference; no wholesale imports. The parent conductor stays read-only during implementation and owns integrated verification and documentation.

Initial evidence is under .flow/tmp/qa-live-20260908/visual-verified/. In meeting-live/header.black-gold.1x.window.png the first segment's timestamp/first text line is clipped although the transcript fits the viewport. The new-meeting rail elides the identifying start of a long engine label. History DetailBlock/AudioStrip and NotesEditor use heavier border/fill than the source artboards. AnalysisRail uses a top Column and bottom facts without a scroll viewport; reproduce long-content/short-window overlap before fixing it.

## Acceptance
- [ ] Reproduce and fix the first live-transcript row clipping. Preserve tail-following during actual capture and usable scrolling; cover resize/content-fit cases with a focused regression.
- [ ] Keep long analysis readable and separate from footer facts at short window heights; reproduce any overflow before editing and preserve analysis actions.
- [ ] Keep the identifying portion of engine/fact values readable in the meeting rail, through wrapping or another justified layout treatment without inventing labels or replacing runtime facts.
- [ ] Restore subtle content-panel chrome where original HTML specifies hairlines/transparent backgrounds; retain control focus/hover affordances and deliberate active-theme differences. Inspect meeting-row alignment and fix only evidence-backed mismatches.
- [ ] Render before/after full windows, inspect Black Gold and a light palette, and pass task Quick commands. Keep reference images and approvals unchanged; record intentional remaining differences.

## Quick commands

- `make build-qt`
- `ctest --test-dir build/qt --output-on-failure`
- `scripts/qml-lint.sh build/qt`
- `scripts/lint-qml-tokens.sh`
- `scripts/lint-accessible-names.sh`

## Done summary
Transcript readers now see the complete first segment, follow growing capture text, and keep their place while dragging or holding the scrollbar. Long analysis scrolls above its footer facts, keyboard focus reveals the analysis action, engine values wrap, and History/meeting content panels use the Studio hairline and transparent fill; meeting columns now center vertically.

Verification passed `make build-qt`, all 74 CTest checks, QML lint/format, token lint, accessibility-name lint, and `git diff --check`. Four focused layout tests cover transcript fit/resize/tail growth/scroll gestures, long analysis/facts separation, long engine labels, and focus plus invocation of the analysis action after a long failure. The initial three regression failures are in `red-layout.log`; the focus failure is in `red-interactions.log`. A temporary version with the gesture guards removed reproduces a scrollbar-held jump from 637 to 800 in `red-scrollbar.log`; the final tests pass in `green-interactions.log`.

baseline: green via handoff (task .2 implementation 3703d3ef and receipt b29bbf50 passed identical Qt build, 74 CTest checks, QML/token/accessibility commands; intervening 2503c3ea changed only task metadata). Baseline lint/format ran again and passed.

Visual evidence is under `.flow/tmp/beauty-20260908/transcripts-meetings/`. Full-window before/after live, meeting detail and list renders cover Black Gold and Catppuccin Latte. The short-window harness captures the unchanged full AppWindow at 1280x460 with a long summary and the pre-change AnalysisRail loaded from base commit 2503c3ea, then swaps to the corrected rail. Both palettes show the old text/facts collision and the corrected clipped viewport. The short harness and legacy component remain untracked temporary evidence.

History pre-change evidence is `.flow/tmp/qa-live-20260908/visual-verified/history/detail.black-gold.1x.window.png` and `detail.catppuccin-latte.1x.window.png`. The attempted new before-History captures exited 2 because `history` was incorrectly supplied as a meeting state; their failure logs are retained and are not passing render evidence. Final after-History captures use the History route with a valid inert meeting state and exited 0. Final full-window renders for all four surfaces and both palettes passed the host style check.

Pixel comparison against original `docs/design/studio/baselines/history.png` confirms the Black Gold panel border changes from RGB 90,85,70 to 44,42,36, matching the original Studio border. Notes interior changes from 20,20,18 to the page's 13,13,13; in Latte it changes from 233,235,239 to the page's 239,241,245. Notes focus styling and audio/control hover styling remain intact.

Active theme typography, runtime engine/backend labels, seeded dates/counts, waveform data, and functional action states intentionally differ from illustrative Studio facts. No shared Theme tokens, reference images, thresholds, approvals, Home/first-run implementation, or fn-38 state changed. The conductor owns the broad live GUI/matrix and repository-wide build/test/lint verification and documentation. Nothing was pushed.

stage: impl-review - skipped(config: REVIEW_MODE=none)
stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: ca12dbacd59733fea6f32897bbe056ae23bbe348
- Tests: baseline: green via handoff (3703d3ef + b29bbf50; metadata-only 2503c3ea); baseline QML/token/accessibility lints passed, make build-qt, ctest --test-dir build/qt --output-on-failure (74/74 passed), scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, git diff --check, QT_QPA_PLATFORM=offscreen DETTIVO_OMARCHY_THEME_DIR=/nonexistent build/qt/qml/Dettivo/tests/dettivo-quick-test -input qt/qml/Dettivo/tests/qml/tst_meeting_layout.qml (6 passed, including setup/cleanup), Regression evidence: red-layout.log 3 intended failures; red-interactions.log focus failure; red-scrollbar.log guard-removed contentY 637 -> 800; green-interactions.log final pass, Full-window host renders: live, detail-transcript, list, history; black-gold and catppuccin-latte; 8 passed, Short-window full AppWindow 1280x460 before/after render harness, black-gold and catppuccin-latte; passed, Initial before-History render INCONCLUSIVE: invalid DETTIVO_E2E_MEETING_STATE=history, exit 2; original verified pre-change History images used instead, Evidence directory: .flow/tmp/beauty-20260908/transcripts-meetings
- PRs: