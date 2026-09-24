# Beauty pass: every surface walked against the design checklist, the copy, the states, the light theme and the icon, with Gordon's review and re-approved baselines

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-36 row: "`beauty-pass-every-surface` | phase 5 | depends on S-22, S-28, S-35 | Principle 13; FR-V7, V9; walkthrough of every surface against `docs/design/checklist.md` with Gordon's review; copy tone; states per `states-and-hint-sheet.png`; light theme per `home-light-catppuccin-latte.png`; icon per `icon-and-osd-elsewhere.png` | Re-approved baselines for every surface; checklist receipt"
> masterplan Principle 13: "Beautiful by default, never by accident. No stock toolkit look ever reaches a user: every control is drawn by the Dettivo style, every state has a designed transition, every surface follows the active theme. A screen without an approved design baseline is not done. Restraint is part of beauty: fewer elements, more whitespace, one accent, no decoration that carries no information."
> masterplan FR-V7: "Every screen has designed empty, loading, error and first-run states. No screen ever shows a raw table, a raw path or a generic placeholder."
> masterplan FR-V9: "Keyboard-first interaction: every screen is fully operable without a mouse, focus rings use the shell's focus tokens, and the app follows Omarchy conventions (`Super+F` fullscreen, `Escape` closes, `/` searches) with a which-key style hint sheet."
> masterplan 8.10 States and hint sheet: "Nine states with the rule "state sentence, reason, action": empty history, empty meetings, no search results, engine loading with elapsed time, engine crashed with restart and log, download checksum failure with quarantine, no microphone, insertion fell back to clipboard, meeting recovered. The keyboard hint sheet lists the in-app keys and defers the global ones to Hyprland."
> masterplan 8.10 Light theme: "Home re-skinned with Catppuccin Latte's palette and blue accent through the same tokens; nothing else changes." and Icon: "The six-bar mark at 128, 64, 32 and 16 px, dark and light, square and unrounded; the pill on GNOME in a plain always-on-top window with the system UI font and the built-in palette"
> masterplan 14: "When the v1 surface is complete, a cleanup phase turns the gates back on (plan and implementation review, `pipeline.qa`, blocking visual regression, the beauty pass S-36, docs lint) and works through what they find."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Every surface is looked at once, whole, before v1. `docs/design/checklist.md` becomes the instrument: the Studio rules as numbered items, each naming the machine check that proves it where one exists (the token, icon, style, accessible-name and release-text lints, the visual job) so the walkthrough spends its judgment only where no lint can. `dettivo-qa beauty` renders every manifest surface and state on the five palettes into one contact sheet per surface and a report that pre-fills the machine verdicts and leaves the human items open. The pass adds what the sheet demands and the build skipped: the nine designed states and the hint sheet as rendered manifest entries, a copy lint over every user-visible string, a keyboard-only drive through every route, the light theme and the icon held to their artboards. Gordon walks the sheets and the report; his findings become tasks here, the fixes land, and every surface's baselines are re-approved under his name. Pilot never judges beauty: the review is a `NEEDS_HUMAN` checkpoint, and the spec closes on his receipt. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Checklist (`docs/design/checklist.md`, authored here since the repository has none yet): items `C-01` onward grouped as tokens (colour only from the theme, one accent, hairlines, radius 0, spacing tokens, the type scale with tabular numerals), controls (Dettivo style only, the shell's fill and border alphas, focus ring from the focus tokens), states (every screen's empty, loading, error and first-run states follow "state sentence, reason, action"; no raw table, path or placeholder), copy (sentence case, one sentence in the pill, no spinner, the reason and the action named), motion (the library's durations, reduced motion honoured), keys (`Super+F`, `Escape`, `/`, `?` for the hint sheet, every element reachable by Tab), icon (the 16 px grid, 1.5 px strokes, the mark at four sizes); each item carries `check:` with the command or `human` and a `receipt` section at the end with date, reviewer and the surfaces walked. [inferred]
- `dettivo-qa beauty` (`crates/dettivo-qa/src/beauty.rs`): reuses the visual verb's renderer over `qa/visual/manifest.toml` to draw every surface and state on the five palettes at 1x, tiles them per surface into `qa-evidence/beauty/<surface>.png` (states down, themes across, labelled), runs every `check:` command and writes `beauty-report.md` with a section per surface: the checklist items with `pass`, `fail` (the lint's line) or `[ ]` for human items, the artboard scores from the visual report, and the strings the surface shows. `--themes all` on this desktop adds a second sheet across every theme installed under Omarchy's themes directory (colours only, read at run time, never copied into fixtures) as review evidence for FR-V11. [inferred]
- States: the app gains `DETTIVO_E2E_STATE=<name>` for the nine sheet states and `hint-sheet`, each rendered with sample facts and no daemon through `--render`; a manifest surface `states` cites `states-and-hint-sheet.png` with `crop = "boxes"` so each state diffs against its box; the shared `StateView` component (state sentence, reason, action slots) is what every route's empty, loading and error states are built on, and a Qt Quick Test fails every `StateView` instance with an empty reason. It also fails a missing action unless the state is loading or is one of the three explicitly approved actionless states. [inferred]
- Copy lint (`scripts/lint-copy.sh`, run by `just lint-qml`): over the same QML literals the release-text lint reads, a sentence-cased first word, a full stop on every multi-word sentence, no exclamation mark, no ellipsis, no `Please`, no `Oops`, no `Error:` prefix, and a one-sentence limit on strings the pill shows (files under `components/Osd*`); each finding names file, line and rule. [inferred]
- Keyboard drive `keyboard_only`: on both drivers, for every route, Tab and Shift+Tab visit every interactive element of the tree (the `a11y_tree` walk of the GUI pack decides which), the focused element carries the focus ring (its `focus` state read through the tree), `Super+F`, `Escape`, `/` and `?` do what FR-V9 says, and the hint sheet lists the in-app keys with the global ones deferred to Hyprland; fails naming the route and the unreachable element. [inferred]
- Light theme and icon: every manifest surface's `catppuccin-latte` and `builtin-light` entries are re-approved after the fixes; Home's light crops are re-cut from `home-light-catppuccin-latte.png`; a manifest surface `icon` renders the mark at 128, 64, 32 and 16 px on the dark and light palettes through `dettivo-sheet --render --icon` with crops from `icon-and-osd-elsewhere.png`, and the packaged PNG icons are diffed against the same crops in the install test; the pill's off-Omarchy state is the existing `osd` entries on `builtin-dark` and `builtin-light` with the system UI font. [paraphrase]
- Human gate: the last task of this spec is the checkpoint: pilot ends its tick with `NEEDS_HUMAN` naming the contact sheets and the report; Gordon's approval is his receipt in `docs/design/checklist.md` and the re-approved rows in `docs/design/baselines.md`, both committed under his name; findings he records are worked as tasks before the receipt. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo-qa beauty [--surface <name>] [--themes fixtures|all] [--out <dir>]`, `just qa-beauty`; `DETTIVO_E2E_STATE=<state>` (QA mode, documented in `docs/app.md` and `docs/qa.md`); `dettivo-sheet --render --icon`; `scripts/lint-copy.sh`. [inferred]
- Manifest surfaces `states` and `icon`; the `keyboard_only` scenario joins the GUI pack's `gui-history` steps as a must-pass. [inferred]
- No daemon methods, no config keys. [inferred]

## Edge Cases & Constraints

- A finding that needs a design change: the artboard is updated on the canvas, re-rendered and re-cut per `docs/design/README.md`, then the baseline re-approved; the checklist never bends to the implementation. [paraphrase]
- A theme on this desktop that no fixture covers: the all-themes sheet shows it; a defect there is fixed in the tokens, and the fixture set is not grown here. [inferred]
- A string the copy lint flags that is quoting the user or the daemon verbatim (a device name): exempt through a `//: verbatim` marker on the line, counted in the report. [inferred]
- Reduced motion: the states render identically; the contact sheets are stills, so motion is judged from the drives' pacing evidence and the motion table, not from the sheets. [inferred]
- Pilot reaching the checkpoint without the receipt: `NEEDS_HUMAN` every tick, never a pass. [paraphrase]

## Acceptance Criteria

- **R1:** `docs/design/checklist.md` exists with numbered items each naming its check or `human`, and `dettivo-qa beauty` writes one contact sheet per manifest surface on the five palettes plus `beauty-report.md` with the machine verdicts filled and the human items open; unit tests cover the tiling and the report. Errors: a surface in the manifest without a sheet, or a `check:` command that is not found, fails by name. [inferred]
- **R2:** The nine states and the hint sheet render through `DETTIVO_E2E_STATE`, pass the visual job against their boxes on the five palettes at both scales, and the `StateView` test fails every instance missing its reason. It also fails a missing action unless the state is loading or is `search-no-results`, `microphone-missing` or `insertion-fell-back`. State sentences use heading size and emphasis weight, as Gordon approved on 2026-09-13. Errors: a missing crop fails by name. [paraphrase]
- **R3:** `scripts/lint-copy.sh` runs in `just lint-qml`, passes on the branch, and fails a planted `Oops!`, a planted lowercase sentence and a planted two-sentence pill string, naming file, line and rule. Errors: as stated. [inferred]
- **R4:** `keyboard_only` passes on both drivers under Xvfb and here for every route, with the focus ring on the focused element and the four conventions and the hint sheet working. Errors: an element Tab cannot reach fails naming the route and the element. [paraphrase]
- **R5:** Every manifest surface passes the visual job on `catppuccin-latte` and `builtin-light` against re-approved baselines, the `icon` surface passes against the artboard's crops on both palettes, and the packaged PNG icons match the same crops in the install test. Errors: an unapproved difference blocks by surface and theme. [paraphrase]
- **R6:** `NEEDS_HUMAN` checkpoint: Gordon has walked the contact sheets and the report, every finding he recorded is closed as a task here, the checklist's receipt names the date and the surfaces, and every surface's re-approved rows in `docs/design/baselines.md` carry his name; pilot reports `NEEDS_HUMAN` on this item until then and never marks it done itself. Errors: a missing receipt or an open finding keeps the spec open. [inferred]
- **R7:** `docs/design/README.md` moves every round-two surface to `Approved` with its baselines, `docs/app.md` and `docs/qa.md` document the state variable, the beauty verb and the copy lint, and an ADR records the checklist as a gate turned on per ADR 0012 and the human checkpoint rule. Errors: `just docs` fails on an unindexed ADR. [paraphrase]

- **R8:** The release package uses a valid native library layout, installs every required QML module and both package license paths, and selects only the current application package for install tests. `just package-bin` and `just install-test` pass with a working namcap check and all seventeen required install checks. Errors: debug-symbol or ambiguous package selection, namcap execution failure, a namcap error, a missing module or an incomplete install receipt fails explicitly. This release follow-up comes from Gordon's request to merge and release for real all-day meeting use.

## Boundaries

- No new surfaces or features; the pass polishes what v1 has. [paraphrase]
- No growth of the theme fixture set or the visual job's matrix beyond the `states` and `icon` surfaces (S-35 owns the job). [inferred]
- No automated judgment of beauty; the lints prove the rules that have one, Gordon proves the rest. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Beauty erodes one PR at a time under an agent-driven build; the pass is the moment the whole product is looked at as one thing, with the rules written down and the human judgment recorded where it belongs. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** every surface, state, theme and the icon held to the checklist and re-approved. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the keyboard drive and the copy lint turn two design rules into evidence. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-38-beauty-pass-every-surface-against-the.1 |
| R2 | fn-38-beauty-pass-every-surface-against-the.1, fn-38-beauty-pass-every-surface-against-the.2 |
| R3 | fn-38-beauty-pass-every-surface-against-the.1 |
| R4 | fn-38-beauty-pass-every-surface-against-the.1 |
| R5 | fn-38-beauty-pass-every-surface-against-the.1, fn-38-beauty-pass-every-surface-against-the.2 |
| R6 | fn-38-beauty-pass-every-surface-against-the.1, fn-38-beauty-pass-every-surface-against-the.2 |
| R7 | fn-38-beauty-pass-every-surface-against-the.1, fn-38-beauty-pass-every-surface-against-the.2 |
| R8 | fn-38-beauty-pass-every-surface-against-the.3 |

## Approved visual decision, 2026-09-13

Gordon Mickel said, "i have reviewed the package and am happy with all for now", after reviewing the Heimdall package. The [checklist receipt](../../docs/design/checklist.md#approval-evidence-2026-09-13) records the exact package identity and all 18 surfaces. His approval accepts the heading-sized state sentences and the three explanatory actionless states. R2's unconditional action wording is revised above to match that supplied product decision. R4's keyboard requirements remain unchanged.

## Release follow-up, 2026-09-14

The first full package rehearsal exposed stale manifest entries, native QML libraries under the architecture-independent share directory, a package-specific license path gap, and an install recipe that selected the generated debug-symbol package. These are release blockers to the existing implementation, not new UI features. R8 tracks their repair and installed-package proof.
