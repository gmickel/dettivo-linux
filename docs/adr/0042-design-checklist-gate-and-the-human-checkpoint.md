# 0042. The design checklist is a gate with machine verdicts, and the beauty pass closes on a person's receipt, never on a pilot's

Status: Accepted 2026-09-05

## What this gives you

Every rule of the Studio design that a lint can prove is proven on every gate run, every rule that needs an eye is laid out for that eye on one contact sheet per surface with the machine's verdicts already filled, and the pass over the whole product is recorded by the person who looked, not inferred by the loop that built it.

## Situation

The v1 surfaces landed one spec at a time under an autonomous build (ADR 0040), each held to its own artboard by the visual job (ADR 0021) and to the design tokens by the lints of ADR 0010. Nothing looked at the product whole: the copy across screens, the nine designed states the sheet `states-and-hint-sheet.png` fixes, the light theme, the icon at four sizes, and whether a keyboard alone reaches every control (FR-V9) had no gate and no walkthrough. The masterplan's cleanup phase names the beauty pass (S-36) as the moment the whole product is looked at once, with Gordon's review, before the gates that were off during the fast build turn back on. A pilot tick cannot judge beauty; it can only prove what a lint proves and hand the rest to a person with everything that person needs in front of them.

## Decision

- `docs/design/checklist.md` is the instrument: numbered items `C-01` to `C-25` grouped as tokens, controls, states, copy, motion, keys, icon and theme, each carrying `check: <command>` when a lint or a job proves it or `check: human` when a person decides, and a receipt table at the end that names the date, the reviewer, the surfaces walked, the findings and the outcome. A pass without a receipt is open.
- `dettivo-qa beauty` (`just qa-beauty`) renders every manifest surface and state on the five palettes at 1x through the visual verb's renderer, tiles them per surface into `qa-evidence/beauty/<surface>.png` (states down, themes across, the artboard score under every cell) with `dettivo-visual-diff tile`, runs every distinct `check:` command once from the repository root, and writes `beauty-report.md`: the checklist with `pass`, `fail` (the command's last line) or `[ ]`, then a section per surface with its sheet, the items again, the artboard scores per state and theme, and the strings the surface shows, read from the `<png>.strings.txt` every host now writes beside a render. A surface without a single render fails by name; a `check:` command that is not found fails by name. `--themes all` adds `<surface>.all-themes.png` across every theme under `~/.local/share/omarchy/themes` on the desktop it runs on, colours read at run time and never copied into the fixtures. The verb exits 1 when a machine check fails and runs in the rig's drive job under Xvfb, because one of its checks is a drive.
- Three of the checklist's checks are new and come from this decision. `scripts/lint-copy.sh` holds every `qsTr` string in the module and the apps to the sheet's copy rules (a sentence is sentence case with a full stop; no exclamation mark, ellipsis, `Please`, `Oops` or `Error:` prefix; one sentence per pill string) with a `//: verbatim` marker for a quoted name and a `--self-test` that plants three bad strings and proves each is named by file, line and rule; it runs in `just lint-qml`. The `keyboard_only` drive Tabs through every route reading the focused element through the accessibility tree (the `focused` state is the focus ring), fails naming the route and any enabled interactive element Tab never reaches, and proves `Super+F`, `Escape`, `/` and `?`; rows a list moves with `j` and `k`, tabs a number picks, a row's own actions and the hyperlink inside a link are arrow-navigated and outside the Tab chain. The `states` surface of the manifest renders the nine designed states and the hint sheet through `DETTIVO_E2E_STATE` at the artboard's first box and holds each to its box; the `icon` surface renders the mark at 128, 64, 32 and 16 px and the light variant through `dettivo-sheet --render --icon` against the artboard's crops, and the install test holds the packaged PNGs to the same crops.
- The human checkpoint: the last task of the beauty pass is Gordon's walkthrough of the contact sheets and the report. Pilot ends its tick on that item with `NEEDS_HUMAN` naming the sheets and the report, never a pass; a finding he records is worked as a task before the receipt; the receipt in `docs/design/checklist.md` and the re-approved rows in `docs/design/baselines.md` are his, under his name. A re-approval made to put a render in front of him is recorded under `awaiting review` (`dettivo-qa visual approve --by`), so the reviewer's column stays his to fill.

## Keyboard observation update, 2026-09-12

The keyboard drive identifies controls by observed native object paths while CUA retains changing snapshot tokens for actions. It reads the deepest focused node because Qt can report focus on both a list scope and its active row. CUA key chords carry the key and modifiers separately, including Shift+slash for the question mark. The hint sheet takes shortcut priority so a route's J action cannot prevent the sheet from closing. The fixed microphone indicator is disabled so the accessibility tree describes it as an indicator that cannot be toggled. Focus changes, ambiguous matches and ancestor mapping have regression coverage.

## Sample rendering date, 2026-09-12

Sample renders use Wednesday, 3 September 2025, matching the approved artboard. The sample data passes that reference date to History and Meetings grouping and to disclosure labels, so a rerender on another day preserves the same pixels. Production timestamps and disclosure checks continue to use the real local date.

## Visual approval, 2026-09-13

Gordon Mickel reviewed the Heimdall package and said, "i have reviewed the package and am happy with all for now". The [checklist receipt](../design/checklist.md#approval-evidence-2026-09-13) binds that approval to the package manifest and all 18 surfaces. Agents may transcribe this supplied decision and publish matching renders through `dettivo-qa visual approve --by "Gordon Mickel"`; they cannot invent an approval or extend it to changed pixels.

The accepted state contract uses heading size and emphasis weight for the sentence. A reason is required. An action or key is required unless the state is loading or is one of the accepted actionless states, `search-no-results`, `microphone-missing` or `insertion-fell-back`. This decision supersedes the body-size rule text in the historical canvas export and the earlier unconditional action requirement. The original artboards remain design evidence; the re-approved render matrix carries the accepted appearance.

The visual checkpoint is satisfied by Gordon's receipt. Keyboard operation, native focus observations and runtime motion retain their own gates. The comparator's dimensions, per-pixel colour tolerance and negative canary remain unchanged.

## Consequences

- Native keyboard verification records Hyprland monitor state before launching the app. A session lock or unreadable state fails explicitly, so a locked desktop cannot produce misleading control-reachability failures. Xvfb retains its independent desktop.

- The hint sheet ignores modifier-only presses so the queued Shift event from the opening question-mark chord cannot dismiss it. Ordinary keys still close it. CUA key names are case-insensitive, matching the AT-SPI driver; uppercase letters in a chord do not add an implicit Shift modifier.
- The checklist runs as data: `beauty` parses the items, so a rule added to the Markdown with a `check:` line is a gate on the next run, and one with `check: human` is a box on the next sheet. Twenty-five items exist today, thirteen with a command.
- The sheets are stills. Motion (C-18, C-19) and reduced motion are judged from the drives' pacing evidence and the motion table, not from the sheets.
- Copy that the lint flagged on this branch was fixed rather than exempted: three ellipses in commands became `<path>` and `<host>`, `Running…` became `Running.`; the tree carries no `//: verbatim` marker yet.
- Every custom control took the keyboard in this pass: the sidebar items, the closable chips, the text buttons in the dialogs, the detail's delete, the source check boxes, the settings navigation's config button and the rail's config link carry `activeFocusOnTab`, fire on Return, Enter and Space, and draw the shell's focus ring through `FocusRing`. The drive takes about six minutes under Xvfb across the fifteen routes.
- The designed state's sentence uses the heading size Gordon accepted on 13 September 2026. The canvas export's original body-size rule is superseded by the approval above.
- The matrix grows by ten states on five themes and five icon states on two, with `states` rendered by `dettivo-app` at the artboard's first box (`render_crop`) so one page serves ten crops; the light entries of every surface were re-approved after the copy and focus changes, and Home's light crops were re-cut.
