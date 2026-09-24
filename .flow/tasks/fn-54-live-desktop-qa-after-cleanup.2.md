---
satisfies: [R1, R3, R4]
---
# fn-54-live-desktop-qa-after-cleanup.2 Align Home and first run with the Studio artboards

## Description
Gordon asked to keep iterating on PR #53 until the app is beautiful. Compare Home and the three first-run screens with the original Studio artboard PNGs and their authored HTML, then fix evidenced visual mismatches. Preserve theme-driven typography and honest runtime state. This task covers these screens only; the conductor inspects the other surfaces separately.

**Touches:** qt/qml/Dettivo/app/HomeRoute.qml, qt/qml/Dettivo/app/TodayList.qml, qt/qml/Dettivo/app/InstrumentStrip.qml, qt/qml/Dettivo/app/RightRail.qml, qt/qml/Dettivo/app/RailRow.qml, qt/qml/Dettivo/app/OnboardingRoute.qml, qt/qml/Dettivo/app/KeysStep.qml, qt/qml/Dettivo/app/ModelsStep.qml, qt/qml/Dettivo/app/TryItStep.qml, qt/qml/Dettivo/app/FirstRunFooter.qml, qt/qml/Dettivo/tests/qml/tst_app_home.qml, qt/qml/Dettivo/tests/qml/tst_app_first_run.qml, qt/host/app/sample_data.cpp

Additional authorized Touches: qt/qml/Dettivo/app/ModelRow.qml and qt/qml/Dettivo/components/ListRow.qml. Preserve ListRow's default padding for other consumers and existing keyboard behavior.

Ask the conductor before further expanding this write set; do not edit shared Theme tokens or other screens. Source and reference images are read-only. Existing fn-38 changes may be inspected but its baseline approvals, task state and branch must not be modified.

## Acceptance
- [ ] Compare full-window before/after renders with original Home and first-run artboards. Identify concrete spacing, hierarchy, alignment, wrapping and density differences before editing.
- [ ] Fix justified mismatches without replacing actual daemon facts with sample data, breaking keyboard actions, narrowing supported window sizes or overriding active theme fonts/colors.
- [ ] Retain original artboards, approved render images, comparison thresholds and Gordon's final design approval unchanged. Document intentional differences and remaining approval needs.
- [ ] Add focused lasting layout regression coverage where useful. Pass the Qt build, focused QML tests, QML formatting and token/accessibility lints; save before/after images and a concise handoff. The conductor runs the integrated full gate and broad live QA.

## Quick commands

- `make build-qt`
- `ctest --test-dir build/qt --output-on-failure`
- `scripts/qml-lint.sh build/qt`
- `scripts/lint-qml-tokens.sh`
- `scripts/lint-accessible-names.sh`

## Done summary
Home and first run now follow the Studio artboards more closely. Today rows align with their heading, engine progress has separate tracks, first-run branding sits 8 px higher, model size/status columns stay aligned, and Keys/Try it spacing follows the authored layout.

Compared original PNGs and authored Main, FirstRunKeys, FirstRunModels and FirstRunTryIt HTML before editing. Similar code search extended ListRow with configurable horizontal padding while preserving its default for other consumers, and reused ModelRow, RailRow and the existing first-run frame. The conductor explicitly authorized ModelRow and ListRow and recorded the expanded Touches.

Evidence root: `.flow/tmp/beauty-20260908/home-first-run/`.
- Full-window before images: `before-home.png`, `before-keys.png`, `before-models.png`, `before-try.png`.
- Full-window after images: `after-home.png`, `after-keys.png`, `after-models.png`, `after-try.png`. All final Black Gold renders exited 0.
- Home before image comes from the existing verified seeded render. An initial fresh Home capture exited 2 because the command passed an invalid step name; it was inconclusive and is retained in `before-home.log`. The corrected final capture passed.
- Baseline green: Qt build, 74 ctests, QML format/lint, token lint and accessibility lint.
- Red-to-green regressions: first-run header height was 36 instead of 20 at default theme sizing; separate engine progress track was absent. See `red-first-run.log`, `red-home.log` and `verify-tests.log`.
- Final verification: `make build-qt`, `ctest --test-dir build/qt --output-on-failure` (74/74), `scripts/qml-lint.sh build/qt`, `scripts/lint-qml-tokens.sh`, `scripts/lint-accessible-names.sh`, and `git diff --check` passed. Home move/Enter navigation passed, alongside the existing connected/idle meeting-action test.

Intentional differences remain. Typography follows the active theme, so the fixture uses 20 px titles and 12 px body text instead of Studio's 24/13. The real binding snippet and current actions remain longer than the illustrative original. Audio waveforms reflect the existing live renderer. Try it retains its visible focus border. Sample Home disables Start meeting because its render has no connected daemon; the connected/idle action remains tested. Actual daemon facts, theme fonts/colors, original artboards, approved renders, thresholds, and Gordon's approval were unchanged.

The conductor owns the integrated full repository gate, broad live/theme matrix and final visual approval. This task's focused verification does not claim that approval.

stage: impl-review - skipped(config: REVIEW_MODE=none)
stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 3703d3ef523aa48a8c620b98f375e92a893282dd
- Tests: baseline: green (task Quick commands; 74/74 ctests), make build-qt, ctest --test-dir build/qt --output-on-failure (74/74 passed), scripts/qml-lint.sh build/qt, scripts/lint-qml-tokens.sh, scripts/lint-accessible-names.sh, git diff --check, dettivo-quick-test -input qt/qml/Dettivo/tests/qml/tst_app_home.qml (Home move/Enter passed), dettivo-app --render (home, keys, models, try; Black Gold; final exit 0), initial Home before capture: inconclusive, invalid DETTIVO_E2E_STEP; corrected final passed
- PRs:
