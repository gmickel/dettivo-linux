# Design

Design direction and baselines for Dettivo for Linux. The decisions behind them are [ADR 0010](../adr/0010-studio-design-system.md) (the Studio design system and theme tokens), [ADR 0011](../adr/0011-qa-drives-and-audio-rig.md) (how baselines are verified) and [ADR 0042](../adr/0042-design-checklist-gate-and-the-human-checkpoint.md) (the checklist as a gate and the human checkpoint); this directory holds the approved artefacts those decisions point at, and [checklist.md](checklist.md) is the instrument every surface is walked against.

## Studio

Approved by Gordon on 3 September 2026. Canvas: https://claude.ai/code/artifact/17dda12c-f5da-4a20-9338-af1a0e65db2a

Studio is Dettivo's structure (sidebar, history with detail, three-pane meetings, one recording pill) on Omarchy's material: monospace type, sharp corners, hairline borders, alpha-layered surfaces and a single accent. Every colour, size and spacing resolves from the active Omarchy theme (`colors.toml` for the palette, `shell.toml` for control states, font and spacing), so other themes re-skin the same screens. Off Omarchy the app follows the portal colour scheme with the built-in palette.

Two directions were considered and set aside: A "Instrument" (dense, boxed, terminal-monitor energy) and B "Quiet editorial" (transcript first, large type, almost no chrome). Studio takes B's calm and A's visible state. Both sketches stay on the canvas's "Directions considered" page and in `studio/baselines/direction-*.png`.

## What is here

| Path | Contents |
|---|---|
| `studio/source/*.dc.html`, `studio/source/canvas.json` | The canvas artboards as authored. `Main.dc.html` carries a theme tweak (Black Gold, Matte Black, Tokyo Night) to demonstrate token-driven theming. |
| `studio/baselines/*.png` | The artboards, rendered in the Black Gold theme (one in Catppuccin Latte), 1x. These are the references visual regression compares against (ADR 0021). |
| `studio/baselines/<surface>/<state>.png` | The Black Gold crops of an artboard, cut by `scripts/design/crop-baseline.py` from the manifest's crop rule (the six pill states out of `osd.png` at 2x; the three regions of `history.png` and the four of `home.png` by their rectangles). Re-cut after a re-approved artboard. |
| `studio/baselines/<surface>/<state>.<theme>.<scale>x.png` | Approved renders for every other theme and scale, written by `dettivo-qa visual approve` and logged in [baselines.md](baselines.md). |
| `baselines.md` | Who approved which baseline when, with its score against the artboard. |

## Baselines, round one (approved)

| Baseline | Surface | Frame | What it fixes |
|---|---|---|---|
| `home.png` | Home | 1280 × 820 | Status sentence with hotkeys and target app; the single bordered instrument strip (mode segment, input, idle waveform, primary actions); Today as a plain list; engines and agents rail; the shared sidebar footer. Implemented by `qt/qml/Dettivo/app/` (ADR 0020); the crops under `home/` are what `dettivo-app`'s renders are checked against |
| `history.png` | History list and dictation detail | 1280 × 820 | Three columns; focused search with hit count and 12 % accent highlights; day labels; meetings in the same timeline; Enhanced above Raw (weight 300); audio strip; key-value facts including insertion backend and stop-to-insert time; keyboard footer. Implemented by `qt/qml/Dettivo/app/history/` (ADR 0025); the crops under `history/` are what `dettivo-app`'s renders are checked against |
| `meeting-live.png` | Meeting, live | 1280 × 820 | Recording square, title, per-source meters with device and app, elapsed as display type, Pause and Stop; transcript rows with source labels in speaker colours; provisional segment in weight 300; the labelling-rule sentence; Markdown notes with caret; analysis card; disclosure state. Implemented by `qt/qml/Dettivo/app/meetings/MeetingLive.qml` (ADR 0038); the crops under `meeting-live/` are what `dettivo-app`'s renders are checked against |
| `osd.png` | OSD states and bar glyph | 1280 × 1380 | Six pill states (Listening, Transcribing, Enhancing, Inserted, Copied, Error) at 2x; five bar glyph states with the meeting timer. Implemented by `Dettivo.Osd` (`qt/qml/Dettivo/components/Osd.qml`, ADR 0015); the crops under `osd/` are what its renders are checked against |
| `omarchy-bar-panel.png` | Omarchy bar widget and panel | 1280 × 560 | Widget with active underline while recording; 340 px panel with state header, mode segment, Dictate and Stop, engine, Enhanced and insertion rows, last three items, Open Dettivo. Implemented by `Dettivo.BarGlyph` and `Dettivo.BarPanel` (ADR 0030); the crops under `panel/` are what `dettivo-bar`'s renders are checked against |
| `design-system.png` | Palette, type, spacing, controls, icons, motion | 1280 × 1700 | Sixteen palette roles; type scale; spacing and shape tokens with shell sources; control states with the shell's alphas; inputs, segments, chips, row states; sixteen icons; the motion table |
| `direction-a-instrument.png`, `direction-b-editorial.png` | Low-fi alternates | 900 × 560 | Not for implementation |

## Baselines, round two (Approved 2026-09-13)

Gordon Mickel approved every round-two surface in the Heimdall review package on 13 September 2026, including the states, light Home and icon. His [checklist receipt](checklist.md#approval-evidence-2026-09-13) identifies the package and accepted state contract; the [baseline ledger](baselines.md) records all 710 matrix entries at both scales under his name. Historical `awaiting review` rows remain provenance for earlier proposals. Runtime keyboard verification remains a separate requirement of the beauty pass.

| Baseline | Surface | Frame | What it fixes |
|---|---|---|---|
| `first-run-1-keys.png` | First run, Keys | 1280 × 820 | Compositor detected; four bindings with the commands they run; the exact snippet written; live confirmation when F9 is pressed; Skip and the config path. Implemented by `qt/qml/Dettivo/app/KeysStep.qml` (ADR 0024); the crops under `first-run/` are what `dettivo-app`'s renders are checked against |
| `first-run-2-models.png` | First run, Models | 1280 × 820 | Radio list of speech models with size and one-line reason; download progress with checksum state; optional Enhanced block (local Qwen3 4B or Ollama); Continue disabled until a speech model is ready. Implemented by `ModelsStep.qml` |
| `first-run-3-try-it.png` | First run, Try it | 1280 × 820 | Hold F9, speak, text lands in a large field; insertion backend, stop-to-insert time and mode; Open config.toml and Done. Implemented by `TryItStep.qml` |
| `meetings-list.png` | Meetings list | 1280 × 820 | Weeks as labels; rows with when, title and summary, length, speaker swatches, state chip; new-meeting rail with source checkboxes and live meters, engine, speakers, analysis, disclosure, one primary button, config pointer. Implemented by `qt/qml/Dettivo/app/meetings/MeetingsList.qml` and `NewMeetingRail.qml` (ADR 0038); the crops under `meetings/` are what `dettivo-app`'s renders are checked against |
| `meeting-detail.png` | Meeting detail after stop | 1280 × 820 | Facts line; talk-time bar per renamed speaker; Transcript, Notes and Analysis tabs with Raw and Polished; transcript with speaker names; analysis rail with summary, decisions, action items; notes, export and audio facts; delete as urgent text. Implemented by `MeetingDetail.qml`; the crops under `meeting-detail/` are what `dettivo-app`'s renders are checked against |
| `settings-models.png` | Settings, Models | 1280 × 820 | Three-column settings pattern; four selections (dictation, meetings, Enhanced, idle unload); model table with backend, state, progress and actions; "This route writes" TOML block. Implemented by `qt/qml/Dettivo/app/settings/ModelsSection.qml` (ADR 0033); the crops under `settings/` are what `dettivo-app`'s renders are checked against |
| `settings-hotkeys.png` | Settings, Hotkeys | 1280 × 820 | Bindings table; snippet with rewrite and copy actions; toggles for portal backend, media pause, sounds, evdev; TOML block. Implemented by `HotkeysSection.qml`; the backend is one choice where the artboard draws two toggles, and the fifth row (`Open Dettivo`) has no key in the schema |
| `agents.png` | Agents | 1280 × 820 | Socket facts; REST shim off with port and token rule; MCP host rows with config file and write action; the JSON a host receives; TOML block. Implemented by `AgentsSection.qml` |
| `import-and-disclosure.png` | Import dialog and disclosure dialog | 1280 × 820 | Import mapped onto `transcripts.import` with chunking and time estimate; disclosure with the macOS message, Copy message, Not now, Acknowledge and start. Implemented by `ImportDialog.qml` and `DisclosureDialog.qml`; each dialog's rectangle of the artboard is held against the dialog's own rectangle in the render (`meeting-dialogs/`) |
| `states-and-hint-sheet.png` | Empty, loading and error states; keyboard hint sheet | 1280 × 1200 | Nine states with a heading-sized sentence and reason, plus an action except for loading and the three approved actionless states; the in-app key list; the rules for states. Implemented by `qt/qml/Dettivo/app/StateView.qml`, `StatesPage.qml` and `HintSheet.qml` (ADR 0042); the crops under `states/` are what `dettivo-app`'s renders under `DETTIVO_E2E_STATE` are checked against |
| `home-light-catppuccin-latte.png` | Home in a light theme | 1280 × 820 | Same screen through the same tokens with Catppuccin Latte's palette and blue accent; the crops under `home/light/` are what `dettivo-app`'s Catppuccin Latte render is checked against on every `just test-qt` |
| `icon-and-osd-elsewhere.png` | App icon; OSD outside Omarchy | 1280 × 620 | Six-bar mark at 128, 64, 32 and 16 px in dark and light; the pill on GNOME in a plain always-on-top window with the system UI font. Implemented by `qt/qml/Dettivo/icons/dettivo.svg`, `dettivo-light.svg` and the sheet's `IconSheet.qml`; the crops under `icon/` are what `dettivo-sheet --render --icon` and the packaged PNGs are checked against |

Settings routes General, Polish, Insertion, Meetings and Diagnostics follow the Models and Hotkeys pattern exactly; General's render stands for them under the visual gate (`settings-pattern`). Small dialogs (re-run, export sheet, speaker rename, the meeting export sheet, the stop and delete confirmations) are designed inline against the pattern; the rename popover, the Notes and Analysis tabs and the empty list are approved renders (`meeting-detail-tabs`, `meetings-empty`).

## Rules for new surfaces

1. Mock every screen and state on the canvas with the Studio tokens, in dark and light and at least one non-default Omarchy theme.
2. Gordon approves on the canvas. The approved export lands here as `studio/baselines/<surface>.png` and the artboard source under `studio/source/`.
3. `just qa-visual` renders `qa/visual/manifest.toml` and compares each image with its reference ([docs/qa.md](../qa.md), [ADR 0054](../adr/0054-delete-duplicate-work-and-keep-one-owner.md)). Approved renders require exact dimensions and a maximum per-pixel normalized RMS RGB distance within the unchanged `colour_tolerance` (0.06 by default). Artboard comparisons retain their structural correlation score; that score is advisory when an approved render supplies the gate. Every difference and missing approval remains visible until fixed or explicitly approved by Gordon.
4. No screen ships without an approved baseline (ADR 0010).

## Surfaces under visual regression

Every surface in the manifest now uses the render Gordon approved on 13 September 2026 on every listed theme and scale, including Black Gold. The table retains the original artboard comparisons as historical design evidence. The artboard score is the render's structure correlation against the artboard crop, reported on every theme as evidence for the beauty pass (S-36); the gate is the baseline column.

| Surface | Binary | States | Artboard and crops | Baselines | Themes and scales |
|---|---|---|---|---|---|
| `design-system` | `dettivo-sheet` | `sheet` | `design-system.png`, the whole artboard | Approved renders on every theme, Black Gold included: the sheet scores 0.44 against the artboard (its sections are laid out for the tokens, not the artboard's annotations), so its baseline is the render approved on 4 September 2026 and the artboard score is carried in `baselines.md` until the beauty pass | five themes, 1x and 2x |
| `osd` | `dettivo-osd` | `listening`, `transcribing`, `enhancing`, `inserted`, `copied`, `error` | `osd.png`, the six boxes cut to `osd/<state>.png` (2x) | The crops for Black Gold (0.60 to 0.82); approved renders for the other themes | five themes, 1x and 2x |
| `insert-target` | `dettivo-insert-target` | `empty` | none (a QA window, not a designed surface) | Approved renders | five themes, 1x and 2x |
| `first-run` | `dettivo-app` | `keys-header`, `keys-body`, `keys-footer`, `models-header`, `models-body`, `models-footer`, `try-header`, `try-body`, `try-footer` | `first-run-1-keys.png`, `first-run-2-models.png`, `first-run-3-try-it.png`, each cut into its header (`0, 0, 1280, 195`), body (`0, 195, 1280, 525`) and footer (`0, 720, 1280, 100`) as `first-run/<state>.png`; the render is cut to the same rectangle | The crops for Black Gold (0.62 to 0.98); approved renders for the other themes | five themes, including Tokyo Night, 1x and 2x |
| `home` | `dettivo-app` | `sidebar`, `header`, `today`, `rail` | `home.png` | Artboard crops and approved renders; missing approvals remain failures | five themes, 1x and 2x |
| `settings` | `dettivo-app` | `models-selections`, `models-table`, `hotkeys-bindings`, `hotkeys-snippet`, `agents-panels`, `agents-hosts` | `settings-models.png`, `settings-hotkeys.png`, `agents.png`, two rectangles each right of the section column cut to `settings/<state>.png`; the render is cut to the same rectangle | The crops for Black Gold (0.57 to 0.84); approved renders for the other themes | five themes, 1x and 2x |
| `settings-pattern` | `dettivo-app` | `general-rows`, `polish-rows`, `insertion-rows`, `meetings-rows`, `diagnostics-rows` | none (the pattern's own sections have no artboard) | Approved renders | five themes, 1x and 2x |
| `history` | `dettivo-app` | `list`, `detail`, `facts` | `history.png`, three rectangles cut to `history/<state>.png` (the list column, the detail through the audio strip, the facts with the footer); the render is cut to the same rectangle before comparing | The crops for Black Gold (0.59 to 0.78); approved renders for the other themes | five themes, 1x and 2x |
| `meetings` | `dettivo-app` | `list`, `rail` | `meetings-list.png`, the list column (`200, 0, 760, 820`) and the rail (`960, 0, 320, 820`) cut to `meetings/<state>.png`; the render is cut to the same rectangle | The crops for Black Gold (0.70, 0.85); approved renders for the other themes | five themes, 1x and 2x |
| `meetings-empty` | `dettivo-app` | `list-empty` | none (states-and-hint-sheet.png fixes the state's shape) | Approved renders | five themes, 1x and 2x |
| `meeting-live` | `dettivo-app` | `header`, `transcript`, `rail` | `meeting-live.png`, three rectangles cut to `meeting-live/<state>.png` | The crops for Black Gold (0.56 to 0.85); approved renders for the other themes | five themes, 1x and 2x |
| `meeting-detail` | `dettivo-app` | `header`, `transcript`, `rail` | `meeting-detail.png`, three rectangles cut to `meeting-detail/<state>.png` | The crops for Black Gold (0.69 to 0.90); approved renders for the other themes | five themes, 1x and 2x |
| `meeting-detail-tabs` | `dettivo-app` | `notes`, `analysis`, `rename` | none (designed inline against the pattern) | Approved renders | five themes, 1x and 2x |
| `meeting-dialogs` | `dettivo-app` | `import`, `disclosure` | `import-and-disclosure.png`, each dialog's box cut to `meeting-dialogs/<state>.png`; the render is cut at the dialog's own rectangle (`render_crop`) since the app centres each dialog over the page | The crops for Black Gold (0.73, 0.58); approved renders for the other themes | five themes, 1x and 2x |
| `panel` | `dettivo-bar` | `idle`, `recording`, `transcribing`, `meeting`, `hint` | `omarchy-bar-panel.png`, the panel box cut to `panel/<state>.png` (the same rectangle for every state) | The crop for `meeting` on Black Gold (0.60); approved renders for the other states on Black Gold (idle 0.47, recording 0.36 and transcribing 0.47 against the meeting crop, hint 0.03) and for the other themes | five themes, 1x and 2x |
| `bar-glyph` | `dettivo-bar` | `idle`, `listening`, `transcribing`, `meeting`, `error` | `osd.png`, the five cells of the glyph strip cut to `bar-glyph/<state>.png` | The crops for Black Gold (0.60 to 0.74); approved renders for the other themes | five themes, 1x and 2x |
| `states` | `dettivo-app` | the nine states and `hint-sheet` | `states-and-hint-sheet.png`, every box cut by its rectangle to `states/<state>.png`; the render is cut at the artboard's first box (`render_crop`), where the page draws the state named by `DETTIVO_E2E_STATE` | The crops for Black Gold (0.56 to 0.85); approved renders for the other themes | five themes, 1x and 2x |
| `icon` | `dettivo-sheet --icon` | `icon-128`, `icon-64`, `icon-32`, `icon-16`, `icon-light` | `icon-and-osd-elsewhere.png`, the mark's rectangles cut to `icon/<state>.png`; the sheet draws the mark at the same positions | The crops for Black Gold (0.63 to 0.85); an approved render for the light palette (0.63 to 0.85 against the same crops); the packaged PNGs score 0.62 to 0.84 in the install test | `black-gold` and `builtin-light`, 1x and 2x |

The five themes are the fixtures `black-gold`, `catppuccin-latte` and `tokyo-night` under `qt/fixtures/themes/` (Omarchy's `colors.toml` and `shell.toml`, the Black Gold accent set to the artboards' `#F5BF03`) and the built-in palettes `builtin-dark` and `builtin-light` the product shows off Omarchy.

## Rendering

Artboards are rendered with headless Chromium from the artboard source using the Cascadia Mono web font as a stand-in for CaskaydiaMono; the QML implementation uses the Omarchy font alias, so glyph metrics differ slightly from these PNGs, which the structure comparison absorbs. `Main.dc.html` is rendered with its Black Gold tweak values substituted. Re-render after any approved change, then re-cut the crops with `scripts/design/crop-baseline.py`:

```bash
chromium --headless=new --disable-gpu --hide-scrollbars --window-size=1280,820 \
  --screenshot=studio/baselines/home.png file://$PWD/studio/source/Main.dc.html
```
