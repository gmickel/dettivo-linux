# Design checklist

The Studio rules as numbered items, one instrument for looking at every surface whole. Each item names the command that proves it where a machine can, so a walkthrough spends its judgment only where no lint reaches. `dettivo-qa beauty` renders every manifest surface and state on the five palettes into one contact sheet per surface under `qa-evidence/beauty/`, runs every `check:` command below, and writes `beauty-report.md` with the machine verdicts filled and the human items left open; the receipt at the end of this file is where the walkthrough is recorded ([ADR 0042](../adr/0042-design-checklist-gate-and-the-human-checkpoint.md)).

An item reads `check: <command>` when a command proves it (exit 0 is a pass, anything else a fail with the command's last lines in the report) and `check: human` when a person decides. The commands run from the repository root.

## Tokens

- **C-01** Every colour resolves from a `Theme` token; no hex or named colour literal in a component, a styled control or a screen.
  check: scripts/lint-qml-tokens.sh
- **C-02** One accent per theme: the accent colour appears on the primary action, the selected segment, the search highlight and the loading thread, nowhere as decoration.
  check: human
- **C-03** Hairlines are one pixel from `Theme.roleHairline`; borders take the shell's border alpha; nothing is drawn with a shadow, a gradient or a glow.
  check: human
- **C-04** Radius comes from the shell (`Theme.radius`); Black Gold renders square and no surface rounds on its own.
  check: scripts/lint-qml-tokens.sh
- **C-05** Every size and gap is a spacing token (`Theme.space1` to `space8`) or a named geometry token; no bare pixel literal in a size property.
  check: scripts/lint-qml-tokens.sh
- **C-06** The type scale is the theme's monospace face at display, title, heading, body, caption and label sizes; numerals that line up (elapsed, clocks, sizes, counts) use tabular figures.
  check: human

## Controls

- **C-07** Every control is drawn by the `DettivoStyle` module; a control from another style or a text in another font fails the render.
  check: cargo run -q -p dettivo-qa -- visual --surface design-system --theme builtin-dark --scale 1 --out qa-evidence/beauty/style-check
- **C-08** Control states use the shell's fill and border alphas for normal, hover, focus, selected and pressed; nothing invents its own.
  check: human
- **C-09** The focused element carries the focus ring from the focus tokens, and every element a user acts on can take focus from the keyboard.
  check: cargo run -q -p dettivo-qa -- drive keyboard_only --driver atspi
- **C-10** Every interactive element carries an accessible role and a fixed name from the documented list.
  check: scripts/lint-accessible-names.sh

## States

- **C-11** Every screen's empty, loading, error and first-run states are a `StateView`: the state sentence first in heading size and emphasis weight, the reason second in muted, the action third as a key or a small button. Loading states may wait without an action; `search-no-results`, `microphone-missing` and `insertion-fell-back` use the approved explanatory copy without a separate button or key.
  check: cargo run -q -p dettivo-qa -- visual --surface states --out qa-evidence/beauty/states-check
- **C-12** No screen ever shows a raw table, a raw path or a generic placeholder; every error names the process or file involved so the CLI and the journal say the same thing.
  check: scripts/lint-release-text.sh
- **C-13** Loading states show the shimmer thread and an elapsed time; nothing spins.
  check: human
- **C-14** The urgent colour appears only as the 8 px square, never as a background or a text colour.
  check: human

## Copy

- **C-15** Sentences are sentence case with a full stop; no exclamation mark, no ellipsis, no `Please`, no `Oops`, no `Error:` prefix.
  check: scripts/lint-copy.sh
- **C-16** The pill says one sentence per state; the reason and the action are named where a state has them.
  check: scripts/lint-copy.sh
- **C-17** Labels are tracked small caps in the label size; a heading is a sentence, not a category.
  check: human

## Motion

- **C-18** Every transition uses a `Motion` duration and curve from the motion table; reduced motion drops the transition and keeps the state.
  check: human
- **C-19** The recording pill drops no frames while it animates (judged in the drives' pacing evidence, not on the sheets).
  check: human

## Keys

- **C-20** `Super+F` toggles fullscreen, `Escape` closes what is open, `/` goes to search, `?` opens the hint sheet, and Tab reaches every element the a11y walk names on every route.
  check: cargo run -q -p dettivo-qa -- drive keyboard_only --driver atspi
- **C-21** The hint sheet lists the in-app keys and defers the global ones to Hyprland; it closes on any key.
  check: human

## Icon

- **C-22** Every icon sits on the 16 px grid with a 1.5 px stroke and square caps, drawn in the recolour key so the theme colours it.
  check: scripts/lint-icons.sh
- **C-23** The six-bar mark reads at 128, 64, 32 and 16 px in dark and light, square and unrounded; the packaged PNGs match the artboard's crops.
  check: cargo run -q -p dettivo-qa -- visual --surface icon --out qa-evidence/beauty/icon-check

## Theme

- **C-24** Every surface re-skins through the same tokens on every theme: the light palettes change nothing but colour, and a theme without a `shell.toml` still renders as itself.
  check: human
- **C-25** Restraint: fewer elements, more whitespace, one accent, no decoration that carries no information.
  check: human

## Receipt

The walkthrough is recorded here, once per pass, by the person who walked the sheets. A pass without a receipt is open ([ADR 0042](../adr/0042-design-checklist-gate-and-the-human-checkpoint.md)).

| Date | Reviewer | Surfaces walked | Findings | Outcome |
|---|---|---|---|---|
| 2026-09-13 | Gordon Mickel | All 18 manifest surfaces listed below, fixture themes at 1x and 2x | State headings and the three actionless states accepted; no visual changes requested | Approved visual design and baseline re-approval |

### Approval evidence, 2026-09-13

Gordon reviewed `~/Downloads/Dettivo-review-2026-09-12` on Heimdall and stated in the current conversation:

> i have reviewed the package and am happy with all for now

The package contains the 18 contact sheets, the beauty and strict visual reports, and all 710 visual renders for code `c75a043c`. Its `MANIFEST.json` SHA-256 is `46bc4a7bb6b53eec5a3cab01063a5c42b73d9bd5c52b481e620706e4ebd307aa`. All 4,820 listed files matched their hashes; all 710 fresh renders on the approval date matched the package byte for byte before baseline publication.

Surfaces reviewed and re-approved are `design-system`, `osd`, `panel`, `bar-glyph`, `insert-target`, `first-run`, `home`, `history`, `settings`, `settings-pattern`, `meetings`, `meetings-empty`, `meeting-live`, `meeting-detail`, `meeting-detail-tabs`, `meeting-dialogs`, `states` and `icon`. The matrix uses Black Gold, Catppuccin Latte, Tokyo Night, built-in dark and built-in light at both scales; the icon uses its manifest's Black Gold and built-in light subset.

Gordon accepted the package's heading-sized state sentences and the explanatory, actionless `search-no-results`, `microphone-missing` and `insertion-fell-back` states. This resolves the visual contract questions presented in the package guide. The original canvas exports remain historical references; this receipt and the named approved renders define the accepted implementation where the exports' rule text differs.

This receipt transcribes Gordon's supplied visual judgment. Runtime keyboard coverage, focus observation and motion measurements still require their own evidence; visual approval does not waive them or close the parent spec. The [baseline ledger](baselines.md) records each approved matrix entry under Gordon Mickel on 2026-09-13.
