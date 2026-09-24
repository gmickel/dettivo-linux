---
satisfies: [R1, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.4 Finish compact settings and new-meeting controls

## Description
Finish the current beauty iteration with compact-layout fixes grounded in the Studio settings/meetings designs. In the Models screenshot the fixed 112-pixel speech selector truncates a normal Parakeet name despite unused horizontal space, the 128-pixel meetings selector truncates its default label, and the idle value has no visible unit. The new-meeting rail has a top controls Column and bottom config footer without a viewport; reproduce whether controls/footer collide at the app's supported minimum height before editing it.

**Touches:** qt/qml/Dettivo/app/settings/ModelsSection.qml, qt/qml/Dettivo/app/meetings/NewMeetingRail.qml, qt/qml/Dettivo/tests/qml/tst_settings_layout.qml, qt/qml/Dettivo/tests/qml/tst_new_meeting_layout.qml

Expanded scope after inspection: qt/qml/Dettivo/app/meetings/SourceRow.qml, qt/qml/Dettivo/app/meetings/RailFactRow.qml, qt/qml/Dettivo/components/FocusRing.qml, qt/qml/Dettivo/CMakeLists.txt, packaging/manifest.txt, qt/qml/Dettivo/tests/qml/tst_app_meetings_rail.qml. These custom controls lack keyboard activation; fix their shared focus/activation rather than special-case one caller. The existing fn-38 FocusRing may be incorporated after inspection, without importing its other changes or approvals.

The minimum-window before capture also reproduced overlapping list heading/search controls and table headings. This related finding is handed to the conductor's next bounded task; it is outside this task's implementation and completion claim.

Use existing theme tokens, settings controls and FocusFlickable. Keep model keys/selection semantics, live actions and config writes unchanged. Source artboards and approved renders remain read-only. No Theme edits, no backend changes, no baseline/tolerance/approval changes. Conductor owns docs and integrated gates; worker owns these files alone.

## Acceptance
- [ ] Reproduce selector truncation with representative real labels and use available row width so common speech-model and default-meeting labels fit at the normal window size. Keep the selection row inside the existing content width and preserve narrow-window scrolling.
- [ ] Show seconds as the unit for the idle-unload field while preserving the integer value and engines.stt_idle_seconds key.
- [ ] Reproduce any new-meeting rail/footer overlap at 1280x410 and the supported minimum width. If reproduced, keep start/disclosure/source controls reachable through scrolling and keyboard focus without changing the normal-size layout or behavior.
- [ ] Tab reaches the non-fixed source checkbox and clickable meeting facts, focus is visible, and Space/Enter invokes the existing action exactly once with fixed/disabled/editing guards retained.
- [ ] Add focused regression coverage and before/after screenshots, including a light palette and the short window. Pass Quick commands and report justified remaining differences. Do not edit sound code just to create a diff.

## Quick commands

- `make build-qt`
- `ctest --test-dir build/qt --output-on-failure`
- `scripts/qml-lint.sh build/qt`
- `scripts/lint-qml-tokens.sh`
- `scripts/lint-accessible-names.sh`

## Done summary
Model selectors now fit the representative Parakeet and default meetings labels, and idle unload explicitly shows seconds without changing its integer value or config key. New-meeting controls scroll above the footer at 1280x410 and the supported minimum width; shared source and fact controls expose keyboard activation with themed focus rings.

baseline: green via handoff (ca12dbac + f8db31d0; identical Qt build and 74 CTest checks); all baseline QML/token/accessibility lints passed.
stage: impl-review - skipped(config: REVIEW_MODE=none)
stage: plan-sync - skipped(policy: conductor owns downstream sync)

Verification: make build-qt, all 74 CTest checks, scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, and git diff --check passed. The new SettingsLayout regression failed before the fix because Parakeet text required 107.8125 pixels versus 86 available. NewMeetingLayout failed before the fix on the short/minimum footer overlap and missing source keyboard focus. Its normal/short/minimum cases now pass, including Tab reveal, Space toggle, Enter activation, speaker editing, and fixed/disabled source guards. A premature focused observation during linking saw the old binary and was inconclusive; it was rerun against the completed build. The first final QML lint caught an unbound test-component ID; adding the Bound pragma fixed it and the affected test plus lint passed.

Screenshots: .flow/tmp/beauty-20260908/compact-controls/ contains before/after normal and short/minimum full windows, black-gold and catppuccin-latte after captures, and short-window focused Start captures. Normal unfocused Meetings images are pixel-identical (ImageMagick AE=0). Captures use local fake data; the conductor owns live-daemon, full-matrix and integrated build/test/lint verification. Original Studio HTML/PNGs and approval/tolerance state remain unchanged.

Related finding handed to conductor: the Meetings list title/search and column headings overlap at the supported minimum width, independently of this rail fix. This is outside the completion claim and remains for task .5.
## Evidence
- Commits: 05390d75066dac55c0fd9fcba7e939ab2bcff697
- Tests: baseline: green via handoff (ca12dbac + f8db31d0); baseline QML/token/accessibility lint passed, make build-qt, ctest --test-dir build/qt --output-on-failure (74/74 passed), scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, git diff --check, SettingsLayout: red label-fit reproduction -> green, NewMeetingLayout: red short/minimum footer overlap and keyboard source -> green normal/short/minimum, Premature focused run during linking: inconclusive (old binary); rerun after completed build passed, Full-window black-gold/catppuccin-latte captures: .flow/tmp/beauty-20260908/compact-controls, Normal Meetings before/after ImageMagick AE=0
- PRs: