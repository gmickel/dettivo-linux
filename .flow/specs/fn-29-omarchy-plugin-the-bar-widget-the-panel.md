# Omarchy plugin: the bar widget, the panel that hosts the pill, and `dettivo setup omarchy`

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-21 row: "`omarchy-plugin-bar-widget-panel-osd-setup` | phase 3 | depends on S-10, S-11, S-17 | FR-B1 to B7 (separate repo `omarchy-dettivo`); baseline `omarchy-bar-panel.png` | `omarchy plugin validate`; Thor screenshot diff against the bar and panel baseline; IPC state assertions; version-skew hint test"
> masterplan FR-B1: "A separate repository `omarchy-dettivo` provides a plugin with a root `manifest.json` (schema version 1), kinds `bar-widget` and `panel`, entry points for the widget and the panel, `keepLoaded` for the panel, category "Developer Tools", and settings schema entries for glyph style, level meter and OSD preference."
> masterplan FR-B2: "The widget shows idle, recording, transcribing and meeting states from the daemon's event stream (a long-lived `dettivo events --follow --json` process), using the shell's `Color` and `Style` singletons."
> masterplan FR-B3: "The panel offers start, stop and cancel dictation, engine and model selection, meeting start and stop with elapsed time, the last three history items, and "open Dettivo", and hosts the OSD component."
> masterplan FR-B4: "The plugin imports the shared `Dettivo` QML module installed by the package at `/usr/share/dettivo/qml`, checks `system.version` on load, and shows an install or upgrade hint instead of failing when the module or daemon is missing or too old."
> masterplan FR-B5: "All state lives outside the plugin checkout so `omarchy plugin update` can fast-forward."
> masterplan FR-B6: "`dettivo setup omarchy` enables the systemd user service, writes bindings, runs `omarchy plugin add` for the widget when the shell is present, and reloads Hyprland and the shell."
> masterplan FR-B7: "No blocking work runs inside QML; the plugin shells out to `dettivo --json` and never touches audio, models or the socket directly."
> masterplan 8.10 Omarchy bar widget and panel: the approved `omarchy-bar-panel.png` baseline (widget in the bar's right section with the active underline while recording; a 340 px panel with state header, mode segment, Dictate and Stop meeting, three key-value rows, the last three items, Open Dettivo with its shortcut; the panel is what the OSD component is hosted in on Omarchy).

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

On Omarchy, Dettivo lives in the bar. A 16 px glyph in the bar's right section shows idle, recording, transcribing and meeting (with the elapsed timer) from the daemon's event stream, gets the shell's active underline while recording, and opens a 340 px panel on click: the state header, the mode segment, Dictate and Stop meeting, engine, Enhanced model and insertion target as three key-value rows, the last three history items, and Open Dettivo with its shortcut. The panel hosts the recording pill, so on Omarchy the shell shows the OSD and `dettivo-osd.service` steps aside. The plugin content is authored and tested in this repository under `omarchy/` and installed by the package at `/usr/share/dettivo/omarchy`; the separate `omarchy-dettivo` repository is a packaging mirror of that directory (its history is written by the release, never by hand), so `omarchy plugin add` works from either the local path or the git URL. `dettivo setup omarchy` does the whole install in one command. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- The shared module carries the surfaces: `Dettivo.BarGlyph` (the six-bar mark at 16 px, five states, the meeting timer) and `Dettivo.BarPanel` (the 340 px panel from the baseline, every control drawn by the Dettivo style, accessible names in `docs/qa/a11y-names.md`) join `qt/qml/Dettivo/components/` with Qt Quick Tests, so the plugin QML is a thin wiring layer and the panel renders in the visual job without Quickshell. The panel embeds `Dettivo.Osd` in its OSD slot; the pill's state model is the same one `dettivo-osd` feeds. [inferred]
- `omarchy/` in this repository is the plugin folder as the shell loads it: `manifest.json` (schemaVersion 1, id `gmickel.dettivo`, kinds `bar-widget` and `panel`, `keepLoaded: true`, `entryPoints.barWidget = BarWidget.qml`, `entryPoints.panel = Panel.qml`, `barWidget.category = "Developer Tools"`, `defaultSection = "right"`, a settings `schema` for glyph style, level meter and OSD preference whose values are written through `dettivo config set omarchy.*`), `BarWidget.qml`, `Panel.qml`, `DettivoState.qml` (a Quickshell `Process` running `dettivo events --follow --json` for the life of the panel, parsing `dictation.state`, `engine.state`, `audio.level`, `meeting.state` and `meeting.segment` into properties; a second short-lived `dettivo --json` call per action), `README.md`, `LICENSE`. The QML imports `Dettivo` from `/usr/share/dettivo/qml` and binds the shell's `Color` and `Style` singletons onto the module's theme so the panel is the bar's palette. `scripts/omarchy/export-plugin.sh` copies the folder into a checkout of `omarchy-dettivo` with the version from `Cargo.toml`. [inferred]
- Version skew: on load the panel runs `dettivo status version --json` once; a missing binary, a daemon older than the version pinned in `manifest.json`'s `omarchy.minDettivo`, or a failed `Dettivo` import (the module missing at `/usr/share/dettivo/qml`) renders one designed hint state (install with `yay -S dettivo-bin`, or upgrade) in the panel and dims the glyph; nothing throws, the shell never logs a QML error. [paraphrase]
- The panel claims `dev.dettivo.OmarchyPanel` on the session bus (a Quickshell `DBusObject`) while it hosts the pill; `dettivo-osd` already exits disabled when it sees that name, and `[omarchy] osd = "service"` keeps the name unclaimed so the layer-shell pill shows instead. [paraphrase]
- `dettivo setup omarchy` (`crates/dettivo-cli/src/setup.rs`, a new compositor arm beside `hyprland`, `sway`, `niri`): enables `dettivod.socket`, writes the Hyprland Lua binding snippet the existing arm writes, runs `omarchy plugin add /usr/share/dettivo/omarchy --enable` when the `omarchy` CLI is on PATH (skipped with a notice otherwise), then `hyprctl reload` and the shell's reload; `--check` reports each step's state as JSON and `--stdout` prints what it would write; `dettivo doctor` gains an `omarchy` row (shell present, plugin enabled, plugin version, panel on the bus). State (the plugin's settings, its cache) lives under `$XDG_STATE_HOME/dettivo/omarchy/` and `config.toml`, never inside the plugin folder. [paraphrase]
- QA: `omarchy plugin validate omarchy/` joins `just lint`; a Quickshell shim under `qt/fixtures/omarchy-shell/` (the `Color`, `Style` and `Process` types the plugin uses) lets `qmllint` and a load test run the plugin QML in CI; the `omarchy_bar` drive scenario on this machine starts a dictation through the CLI, captures the bar region with `grim`, asserts the daemon state through `dettivo dictation status --json` and diffs the crop against the baseline with a tolerance; the version-skew test runs the panel against a shim that answers an old version and asserts the hint. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Config: `[omarchy] glyph = "waveform"` (`waveform` or `dot`), `level_meter = true`, `osd = "panel"` (`panel`, `service`, `off`), `history_items = 3`, `open_shortcut = "SUPER, D"` (the label the panel shows beside Open Dettivo); every key documented in `docs/config.md` and printed by `config print-default`. The manifest's settings schema mirrors these keys and writes them through `dettivo config set`. [inferred]
- CLI: `dettivo setup omarchy [--check] [--stdout] [--no-plugin]`; the panel's actions map to `dettivo dictation start|stop|cancel --json`, `dettivo speech select --json`, `dettivo meetings start|stop --json`, `dettivo history list --limit 3 --json`, `dettivo app open home`. [paraphrase]
- Daemon: no new methods; the widget reads `dictation.state`, `engine.state`, `audio.level`, `meeting.state` and `meeting.segment` from `events --follow --json` and `system.version` once. [paraphrase]
- Install layout: `/usr/share/dettivo/omarchy/` (the plugin folder), `/usr/share/dettivo/qml/` (the module); the `omarchy-dettivo` repository root equals the `omarchy/` folder. [inferred]

## Edge Cases & Constraints

- The daemon is down: the glyph shows idle dimmed, the panel's header says `Daemon unavailable.` with the `systemctl --user start dettivod.socket` hint; the events process restarts with the pill's backoff (half a second to eight). [paraphrase]
- `events.overflow`: the state process re-subscribes and re-reads `dictation.status` rather than showing a stale state. [paraphrase]
- No `omarchy` CLI (plain Hyprland): `dettivo setup omarchy` writes the bindings and enables the units, reports the plugin step skipped, and `dettivo-osd.service` stays the pill. [paraphrase]
- `omarchy plugin update` on the mirror: fast-forwards because the folder carries no state; a local edit in the plugin folder is the user's and the update refuses as the shell does. [paraphrase]
- The panel is open while a meeting runs: the timer ticks from `meeting.state`'s duration, never from a QML clock alone. [inferred]
- Quickshell has no AT-SPI tree: the drive asserts state through the CLI and pixels through the baseline crop, as 9.4 item 5 has it. [paraphrase]

## Acceptance Criteria

- **R1:** `omarchy plugin validate omarchy/` passes in `just lint`, the manifest carries the FR-B1 fields (id, both kinds, both entry points, `keepLoaded`, the category, the three settings entries) checked by a unit test, and `scripts/omarchy/export-plugin.sh` produces a folder that validates and matches `omarchy/` byte for byte. Errors: a manifest field missing or an entry point that does not exist fails the lint naming it. [paraphrase]
- **R2:** `Dettivo.BarGlyph` and `Dettivo.BarPanel` pass Qt Quick Tests for the four widget states, the meeting timer, every panel row and action, the accessible names and the hint state; the plugin QML loads under the shell shim in CI with zero `qmllint` warnings. Errors: a control rendered by a non-Dettivo style fails the style assertion. [paraphrase]
- **R3:** The `omarchy_bar` drive on this machine: a CLI-started dictation turns the glyph to recording with the active underline, the panel's header follows transcribing and inserted, the last three history rows match `dettivo history list --limit 3 --json`, and `dettivo dictation status --json` agrees with every state the screenshot shows; the bar crop diffs against `omarchy-bar-panel.png` within tolerance. Errors: a state the CLI reports that the crop does not show fails naming both. [paraphrase]
- **R4:** With the panel hosting the pill, `dettivo osd status` reports the service host absent with the `dev.dettivo.OmarchyPanel` notice and one pill shows during a dictation; `[omarchy] osd = "service"` restores `dettivo-osd` (a Thor drive asserts both). Errors: two pills or none fails the scenario. [paraphrase]
- **R5:** `dettivo setup omarchy` on a fresh profile enables the socket, writes the snippet, adds and enables the plugin, reloads Hyprland and the shell, and `--check` then reports every step `ok`; without the `omarchy` CLI the plugin step is `skipped` with the reason; the version-skew test shows the upgrade hint against an old `system.version` and the install hint with the module path absent. Errors: a step that fails names the command and its exit code. [paraphrase]
- **R6:** The visual job renders `panel` (states idle, recording, transcribing, meeting, hint) and `bar-glyph` (five states) on the five palettes at 1x and 2x against `omarchy-bar-panel.png` crops and approved baselines recorded in `docs/design/baselines.md`. Errors: an unapproved difference above threshold blocks. [paraphrase]
- **R7:** `docs/omarchy.md` (install, the panel, the settings keys, the mirror repository, `dettivo setup omarchy`), `docs/config.md` for `[omarchy]`, `omarchy/README.md`, and an ADR recording the in-repo plugin folder with the mirror repository and the panel-hosted pill. Errors: `just docs` fails on a missing key or an unindexed ADR. [paraphrase]

## Boundaries

- No AUR package, release workflow or the `omarchy-dettivo` repository's automation beyond the export script (S-31). [paraphrase]
- No meeting GUI in the panel beyond start, stop and the timer (S-27); no settings editing beyond the three keys (S-19). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The bar is where an Omarchy user expects a resident tool to live; a plugin that is authored, linted and rendered in this repository keeps it at the same bar of quality as the app, and the mirror repository is what `omarchy plugin add` needs. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** the widget and panel use the shell's own singletons and the approved baseline, and the pill moves into the shell. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the bar drive asserts state through the CLI and pixels through the baseline. [strategy:Complete speech workflows, proven by drives]

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
