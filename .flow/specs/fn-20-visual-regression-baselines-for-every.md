# Visual regression: baselines for every surface and theme, frame pacing in drives, a canary

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-35 row: "`visual-regression-baselines-frame-pacing` | phase 3 | depends on S-03, S-34 | QA-14, QA-16, NFR-12, NFR-13; perceptual diff job over every surface and theme; frame timing capture in drives | Baseline job green on Xvfb and Thor; deliberate regression caught in a canary test"
> masterplan QA-14: "A visual regression job renders every surface and state at 1x and 2x, dark and light, in the default Omarchy theme plus at least two others, and compares against approved baselines with a perceptual diff; any difference above threshold blocks merge until fixed or re-approved."
> masterplan QA-16: "The drive records scene graph frame timings during recording, transcribing and theme-switch scenarios and fails on dropped frames per NFR-12 and NFR-14."
> masterplan NFR-13: "Zero unapproved pixel differences above the perceptual threshold across the baseline set at release ... NFR-13 is a release blocker from the first visual baseline onward."
> masterplan FR-V11: "Light and dark are both first-class and every Omarchy theme shipped with 4.0.2 is a supported target; screenshots for all of them are part of the visual baseline set."
> masterplan 14: "Keep the gates on from day one: `pipeline.qa on`, visual regression against `docs/design/studio/baselines/` once S-35 lands"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Design stops regressing by accident. The OSD spec built a perceptual diff tool and a per-state visual check for one surface; this spec turns that into the visual regression system the strategy names as a gate: every surface that exists (the design system sheet, the pill's states, the insert target, and the app's routes as they land) is rendered headlessly at 1x and 2x, dark and light, in the default Omarchy theme plus two others, compared with approved crops of the baselines under `docs/design/studio/baselines/` at a perceptual threshold, and any unapproved difference blocks the merge until it is fixed or re-approved through a recorded command. Frame timings are captured in the recording, transcribing and theme-switch drives against NFR-12 and NFR-14, and a canary test proves the job catches a deliberate regression. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `dettivo-qa visual` (a new verb): reads `qa/visual/manifest.toml` listing every surface with its render command (a binary and the QA variables that show a state: `DETTIVO_E2E_OSD_STATE`, `DETTIVO_E2E_OPEN`, the design-system sheet's `--sheet` mode from S-34), its baseline crop(s), the crop geometry, and the surfaces' expected theme set; renders each entry through Xvfb (the session script) at 1x and 2x, dark and light, for the theme fixtures `black-gold` (the baseline theme), `catppuccin-latte` (light) and `tokyo-night` (the canvas's proof theme) under `qt/fixtures/themes/`, diffs with `dettivo-visual-diff` at the per-surface threshold in the manifest (the structure-correlation score the OSD spec introduced), writes `visual-report.json` and a Markdown table with the diff images beside the renders, and exits 1 on an unapproved difference. [inferred]
- Baselines: the approved renders live as crops under `docs/design/studio/baselines/<surface>/<state>.png` (the OSD spec's layout), cut from the artboard PNGs by `scripts/design/crop-baseline.py` (the OSD crop script generalised with the manifest's geometry); the baselines are the design canvas exports for surfaces with an approved artboard, and for other themes and scales the first accepted render becomes the baseline through `dettivo-qa visual approve <surface> [--theme --scale]`, which copies the render into place and records who and when in `docs/design/baselines.md`. [inferred]
- Gate: the CI job `visual` runs `just qa-visual` after the drives and blocks the merge on a failure (the fast-build phase kept it advisory; this spec makes it blocking, as the strategy's cleanup names); on this machine `just qa-visual` is the same command. Re-approval is a commit that updates the crop through the approve command, reviewed like any other change. [paraphrase]
- Frame pacing: the OSD spec's pacing collector becomes a shared host component (`qt/host/pacing`) that any Qt binary enables with `DETTIVO_QA_PACING=<file>`; the drives `osd_dictation` (recording and transcribing) and a new `theme_switch` drive (replace the theme files the way `omarchy theme set` does while the app and the pill are visible) record swap intervals and render cost, and fail when dropped frames exceed the NFR-12 budget at the display's refresh rate or the palette applies later than 100 ms; on CI the numbers are recorded and the gate is the presence of the report (Xvfb has no real refresh), on this machine the thresholds gate. [paraphrase]
- Canary: `dettivo-qa visual --canary` renders one surface with a deliberate token change (an accent nudge through an environment override the theme object honours only under QA mode) and asserts the diff fails; the CI job runs it after the real pass so a silently broken diff never passes green. [paraphrase]
- Negative style check (QA-17, from S-34) runs as part of the visual verb's render pass: any control instance with a non-Dettivo style or a Qt default font family in the rendered tree fails the entry. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo-qa visual [--surface <name>] [--theme <name>] [--scale 1|2] [--out <dir>] [--json]`, `dettivo-qa visual approve <surface> [--theme --scale --state]`, `dettivo-qa visual --canary`; `just qa-visual`, `qa-visual-approve`; the manifest format documented in `docs/qa.md`. [inferred]
- `DETTIVO_QA_PACING=<file>` for every Qt binary; `pacing.json` shape from the OSD spec with `surface` and `scenario` fields added. [inferred]
- No daemon methods. [inferred]

## Edge Cases & Constraints

- A surface without an approved artboard for a theme: the first accepted render is the baseline and the report marks it `first-approval`; never a silent pass. [inferred]
- Font differences between machines (CaskaydiaMono on this machine, Cascadia Mono in CI): the threshold and the structure-correlation metric absorb glyph rendering; a font family that is not the theme's fails the negative style check instead. [paraphrase]
- Xvfb has no fractional scale: 2x renders use `QT_SCALE_FACTOR=2`. [inferred]
- A missing baseline crop fails by name; a missing theme fixture fails by name. [paraphrase]
- Thresholds are per surface in the manifest, never per run. [inferred]

## Acceptance Criteria

- **R1:** `dettivo-qa visual` renders every manifest surface at 1x and 2x, dark and light, in the three theme fixtures, diffs each against its baseline crop, writes the report with diff images, and exits 1 on an unapproved difference; unit tests cover the manifest parsing, the matrix expansion and the report. Errors: a missing crop or theme fails by name. [paraphrase]
- **R2:** Baselines exist for the design system sheet, the six pill states and the insert target (and the app's Home route once its spec lands, by manifest entry) in Black Gold from the artboards and in the other themes and scales through `approve`, recorded in `docs/design/baselines.md`; the whole set passes on this machine and in CI. Errors: as stated. [paraphrase]
- **R3:** The canary run fails as expected in CI and here, and the CI `visual` job blocks the merge on a real failure (proven once on a throwaway branch with a token change, the evidence linked in the PR). Errors: as stated. [paraphrase]
- **R4:** Frame pacing: the `osd_dictation` and `theme_switch` drives record swap intervals and render cost through the shared collector for the pill and the app, the theme switch applies within 100 ms here, dropped frames stay within the NFR-12 budget at this display's refresh rate, and CI records the numbers with the presence of the report as its gate. Errors: a dropped frame over budget is reported with its timestamp. [paraphrase]
- **R5:** The negative style check runs in the render pass: a planted control with the Basic style and a planted default font family fail their entry by name (unit test with the plant under QA mode). Errors: as stated. [paraphrase]
- **R6:** `docs/qa.md` gains the visual regression section (manifest, thresholds, approve flow, the canary, pacing), `docs/design/README.md` maps every surface to its crops and themes, `.flow/config` or the CI workflow records the gate as blocking, and an ADR records the perceptual metric, thresholds and the approval flow. Errors: as stated. [paraphrase]

## Boundaries

- No new surfaces; the manifest grows as specs land. [inferred]
- No beauty pass or re-approval of designs (S-36). [paraphrase]
- No soak or benchmark timing. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Beautiful is a gated requirement; without a blocking diff job the bar erodes one PR at a time, and the strategy names visual regression as the gate that turns on after the fast build. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** every surface, theme and scale is held to its approved baseline by a job that blocks. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** frame pacing and theme switching become drive evidence with thresholds. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
