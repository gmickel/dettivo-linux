---
satisfies: [R1, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.5 Keep the Meetings list readable at minimum width

## Description
The compact-controls pass reproduced an independent Meetings list failure at the app's supported minimum width. In .flow/tmp/beauty-20260908/compact-controls/before-meetings-minimum.png the title is covered by Search and the table labels overlap because fixed columns exceed available width. Finish this beauty iteration by keeping the list readable at minimum dimensions and preserving the normal Studio layout.

**Touches:** qt/qml/Dettivo/app/meetings/MeetingsList.qml, qt/qml/Dettivo/app/meetings/MeetingsListHeader.qml (only if a split is needed), qt/qml/Dettivo/tests/qml/tst_meetings_list_layout.qml, qt/qml/Dettivo/tests/qml/tst_app_meetings.qml, qt/qml/Dettivo/CMakeLists.txt and packaging/manifest.txt (only if adding a component).

Additional authorized Touches: qt/qml/Dettivo/accessible-names.txt and docs/qa/a11y-names.md to register the new Meetings table viewport. Its keyboard focus must have a visible themed ring anchored to the visible viewport, not the scrolling content.

Reuse existing theme tokens, FocusFlickable and search/list components. A compact header may stack; a horizontal table viewport may preserve the designed minimum column widths. Do not hide fields, increase minimum window dimensions, change data/selection/search/import behavior, edit Theme, or replace original/approved baseline images or thresholds. Stay under the QML file limit; request additional Touches only if needed.

## Acceptance
- [ ] Reproduce title/search overlap and negative title-column width in a focused regression at Theme.appWindowMinWidth and the normal window height/short height.
- [ ] At minimum width, the heading, search and Import controls remain readable and keyboard reachable. All table columns retain positive usable widths and remain reachable through scrolling.
- [ ] Preserve normal 1280x820 geometry and all existing search, j/k/Enter selection, import and empty-state behavior. Normal-size before/after should be pixel-identical unless a documented defect requires otherwise.
- [ ] Capture before/after populated and empty views, Black Gold and a light palette, and pass Quick commands. Document intentional remaining differences; approvals remain unchanged.

## Quick commands

- `make build-qt`
- `ctest --test-dir build/qt --output-on-failure`
- `scripts/qml-lint.sh build/qt`
- `scripts/lint-qml-tokens.sh`
- `scripts/lint-accessible-names.sh`

## Done summary
The Meetings list now remains readable at the supported minimum width. Its compact header stacks and the search field fills the space beside Import. A horizontal viewport preserves the normal table column widths, supports Left/Right and Home/End navigation, and keeps its themed focus ring fixed on the visible viewport.

The worker reproduced a negative title-column width of -123 pixels and captured before/after normal, minimum and short windows with populated and empty data. Its turn ended at a usage limit after implementing the changes. The conductor inspected the actual diff and completed verification inline, without restarting implementation or relying on the inherited pass claims.

Final verification passed make build-qt, all 74 CTest tests, QML lint/format, token lint, accessibility-name lint and git diff --check. The layout tests cover compact header alignment, positive usable column widths, search/import activation, keyboard horizontal navigation and fixed focus-ring geometry, plus the empty state. The route test preserves j/k/Enter and search behavior. Fresh captures under .flow/tmp/beauty-20260908/meetings-list/verified cover Black Gold and Catppuccin Latte; all four normal populated/empty comparisons have ImageMagick AE=0 against the before renders.

No original artboard, approved render, threshold, Theme token, minimum window dimension or data contract changed. Runtime text/font differences remain governed by the active theme and daemon facts. The conductor owns broad live GUI/matrix verification and the final repository gate.

stage: impl-review - skipped(config: repository fast-build mode)
stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 2aa30d515a0fe1a4bd5c530070d916edd1393606
- Tests: make build-qt, ctest --test-dir build/qt --output-on-failure (74/74 passed), scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, git diff --check, Fresh normal/minimum/short populated+empty captures on Black Gold and Catppuccin Latte, Four normal-size before/after image comparisons: AE=0, Worker usage-limit interruption recovered inline; checks independently rerun
- PRs: