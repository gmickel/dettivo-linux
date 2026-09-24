# Design system: theme tokens, Quick Controls style, type, motion and icons

## Conversation Evidence

> user (turn 5): "are we sure about gtk4, what are other apps using like omawriter etc and other omarchy apps we know of"
> user (turn 5): "i would rather not choose the framework based on QA only, instead what is best for linux/omarchy"
> user (turn 8): "it should be MAXIMALLY beautiful"
> user (turn 10): "its amazing"
> user (turn 11): "make sure we have all of these in detail in our plan as screenshots or however you work with the /design thing, it's brilliant"
> user (turn 12): "the black/gold would be adapted to whatever theme the user is using right?"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> user (turn 20): "yes"

## Goal & Context

<!-- Goal & Context: 40% [user], 30% [paraphrase], 30% [strategy] -->

Every Dettivo surface looks like part of Omarchy on whatever theme the user runs, switches with the theme without a restart, and reads as a calm native app on GNOME or KDE. This spec builds the shared QML module that makes that true once, so the app, the OSD and the bar plugin never draw a control of their own. It is built to the approved Studio design system sheet and the approved baselines. [paraphrase]

The user's bar is "MAXIMALLY beautiful" and the direction was chosen on what is best for Linux and Omarchy; the Black Gold renderings are one theme, and the tokens must adapt to whatever theme the user is using. [user]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- A theme object reads the Omarchy theme's colours file and shell file when present: the palette (foreground, background, accent, cursor, selection, the sixteen colours), the per-state fill and border alphas for controls, the font alias and base size, the spacing tokens, corner radius and gaps. It watches the files and re-resolves on change. Off Omarchy it follows the portal colour scheme with the built-in dark and light palettes designed to the same roles. [paraphrase]
- Semantic roles derived once from the palette: text, muted text, surface, raised surface, border, hairline, accent, selected, urgent, and a stable mapping from the palette colours to speaker colours. [paraphrase]
- A custom Qt Quick Controls style covering buttons, text fields, combo boxes, check boxes, radio buttons, switches, scroll bars, menus, popups, tool tips, list rows, tab bars and progress bars, drawing every state with the shell's alpha model. A startup assertion fails when any control renders with a style other than this one. [paraphrase]
- The type scale (display, title, heading, body, caption, tracked label, weight 300 for provisional text), tabular numerals for time, the spacing scale, control height and radius as tokens. [paraphrase]
- A motion library with named easings and durations (enter, exit, reveal, pill spring, level, shimmer) and a reduced-motion switch from the portal setting. [paraphrase]
- An icon set as SVG sources on the 16 px grid with a single stroke width, recoloured by token at runtime, and the six-bar mark used for the app icon and the bar glyph. [paraphrase]
- Reusable components used by every later surface: section label, chip, key cap, status dot, level meter, waveform rendered on the scene graph, segmented control, key-value row, list row with the selection rail. [inferred]
- Every control and component carries an accessible role and a stable accessible name, and a Qt Quick Test per component covers its states and names. A component sheet renders the whole system for comparison with the approved design-system baseline. [paraphrase]

## API Contracts

<!-- API Contracts: 50% [paraphrase], 50% [inferred] -->

- The module is importable by name from the app, the OSD host and the Omarchy plugin, installed to the system QML location by the bootstrap's CMake project. [paraphrase]
- The theme object exposes the roles, tokens and font as read-only properties plus a `source` property naming where they came from (`omarchy` or `builtin-dark` or `builtin-light`). [inferred]
- Accessible names are documented in one list that the accessible-name lint reads. [paraphrase]
- The component sheet is a runnable QML scene the QA runner can screenshot. [inferred]

## Edge Cases & Constraints

- A theme whose accent equals its foreground keeps focus and selection distinguishable through the alpha fills alone, the way the shell does. [paraphrase]
- Corner radius follows the shell token; Black Gold renders square and other themes may round. [paraphrase]
- Missing or malformed theme files fall back to the built-in palette without a visible flash; the first frame already carries the resolved theme. [paraphrase]
- The stand-in web font used for the baselines differs slightly in metrics from the Omarchy alias, so the sheet comparison is advisory during the build and blocking after cleanup. [paraphrase]

## Acceptance Criteria

- **R1:** With Omarchy theme files present, every role and token resolves from them, and replacing the files with another theme's re-resolves palette, font and spacing within 100 ms without restart, verified by a test that swaps the files. Errors: a malformed file falls back to the built-in palette and reports the parse error once. [user]
- **R2:** Without Omarchy theme files the theme object follows the portal colour scheme with the built-in dark and light palettes and reports its source. Errors: an unavailable portal defaults to dark. [paraphrase]
- **R3:** The custom style covers the listed controls with the shell's normal, hover, focus, selected and pressed alphas, and the startup assertion fails the process when a control uses another style. Errors: no error surface beyond the assertion. [paraphrase]
- **R4:** Type scale, spacing, control height, radius and tabular numerals are tokens consumed by every component; no component carries a literal size or colour, verified by a lint over the module. Errors: a literal value fails the lint with the file and property named. [paraphrase]
- **R5:** The motion library exposes the named easings and durations, honours the reduced-motion setting, and the waveform and meter render on the scene graph without a canvas repaint. Errors: no error surface beyond the reduced-motion test. [paraphrase]
- **R6:** The icon set ships as SVG sources with one stroke width, recolours by token at runtime, and includes the six-bar mark at every size the app, the bar and the desktop icon need. Errors: an icon outside the grid fails the icon lint. [paraphrase]
- **R7:** Every control and component has an accessible role and a documented accessible name and a Qt Quick Test covering its states, and the component sheet renders and matches the approved design-system baseline within the advisory tolerance. Errors: a component without a test or a name fails CI. [paraphrase]

## Boundaries

- No application screens, no OSD host process and no bar plugin; those specs consume this module. [paraphrase]
- No sounds; feedback audio belongs to the daemon and the hotkeys spec. [inferred]
- No visual regression job; the module ships a sheet the visual regression spec will compare. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Beauty is a product requirement the user set as maximal, and the direction was chosen for fit with Linux and Omarchy rather than for automation convenience; tokens from the theme are what make one design look native on every theme the user picks. [user]
- One module shared by the app, the OSD and the bar plugin means every fix lands everywhere at once, which is what keeps a fast build from drifting visually. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** this spec is the track's foundation; every value resolves from the theme, held to the approved baselines. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
