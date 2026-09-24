# Qt app shell: window, routes, daemon client, live theme and the Home surface

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-17 row: "`qt-app-shell-routes-theme-watch-a11y-qa-hooks` | phase 3 | depends on S-03, S-07, S-34 | FR-U1, U2, U4, U5, U6, U7, FR-V10, NFR-14, T1, T2, T14, T15, T16; baseline `home.png` (Home route and sidebar) | Xvfb drive opens every route via `DETTIVO_E2E_OPEN`; a11y lint; live `omarchy theme set` test on Thor with timing; visual diff against the Home baseline"
> masterplan FR-U1: "`dettivo-app` is a Qt 6 Quick application (Qt 6.8 floor) with a thin C++ host: window, QLocalSocket JSON-RPC client, list models, settings binding, event subscription. All presentation lives in QML inside the shared `Dettivo` QML module with a custom Qt Quick Controls style."
> masterplan FR-U2: "Routes: onboarding, home, history, history detail, meetings, meeting live, meeting detail, settings sub-routes (general, hotkeys, models, polish, insertion, meetings, agents, diagnostics). Each route is openable by an environment variable in QA mode."
> masterplan FR-U5: "Theme resolution: on Omarchy the app reads `~/.local/state/omarchy/current/theme/colors.toml` and `shell.toml` ..., watches them for `omarchy theme set`, and applies palette and font live; elsewhere it follows the portal `color-scheme` setting and text scale as Omawrite does, with the built-in palette."
> masterplan FR-U7: "The app launches in under 300 ms to first frame on Thor and stays under 60 MB RSS idle, in the spirit of Omarchy's own apps."
> masterplan FR-V10: "Window chrome is compositor-native: no client-side decorations on Hyprland or other wlroots compositors, transparent corners where the compositor rounds, correct fractional scaling on every monitor, and a first frame that already carries the theme (no flash of unstyled UI)."
> masterplan 8.10 Home: "A status sentence, not a dashboard: "Ready." plus the exact hotkeys and the app that will receive text ... The instrument strip holds the mode segment (Raw, Polish, Enhanced), the input name and rate, an idle waveform that becomes the live one, and the two primary actions ... Today is a plain history list ... The right rail shows the four engines with a 2 px progress hairline and the agent surfaces with their last call ... The sidebar footer is the same three rows on every window: active speech engine, language model, socket mode."
> masterplan NFR-14: "Palette and font apply within 100 ms of `omarchy theme set` with no restart and no flash"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

The desktop app exists and looks like Omarchy from its first frame. `dettivo-app` is the thin C++ host from the masterplan (window, QLocalSocket JSON-RPC client with the event subscription, list models, settings binding, theme watcher, QA hooks) around the shared `Dettivo` QML module, with every route of FR-U2 reachable by name and by `DETTIVO_E2E_OPEN`, the sidebar and footer of the approved Studio direction, and the Home route built to `home.png`: the status sentence, the instrument strip with the live waveform and the two primary actions, today's dictations, and the right rail with the engines and agent surfaces. Routes other than Home are scaffolds with their designed empty state that later specs fill. The theme follows `omarchy theme set` live within the NFR-14 budget, and the app opens in under 300 ms with under 60 MB idle. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `qt/apps/dettivo-app` (the skeleton exists): `main.cpp` sets the Dettivo style before any control loads (the startup assertion of FR-V1), reads the QA environment (`DETTIVO_E2E_OPEN`, `DETTIVO_E2E_ROUTE`, `DETTIVO_E2E_COMPLETE`, `DETTIVO_E2E_SEED` passthrough), and hands the host objects to QML. Host classes under `qt/host/app/`: `DaemonClient` (QLocalSocket over the daemon socket with the token, request ids, `events.subscribe` with reconnect and backoff, typed signals for `dictation.state`, `audio.level`, `engine.state`, `job.progress`, `model.download`), `Router` (route enum, history stack, `open(route, arg)`), `HistoryModel` (a `QAbstractListModel` over `transcripts.list` with paging), `EnginesModel` (`speech.providers.list` plus `engine.state`), `StatusModel` (`system.health`, `system.capabilities`, the hotkeys block, the insertion target), `ConfigBinding` (`config.get/set` through the daemon, the effective value with its source), `ThemeWatcher` (reuses the theme object from the design system spec and watches the Omarchy theme files with `QFileSystemWatcher`, falling back to the portal `color-scheme` setting through `ashpd`'s D-Bus interface or `QDBusInterface` off Omarchy), `AppState` (`$XDG_STATE_HOME/dettivo/state.toml`: window geometry, last route, first-run progress). [paraphrase]
- QML under `qt/qml/Dettivo/app/`: `AppWindow.qml` (frameless on wlroots, compositor-native chrome, first frame with the theme applied, `Super+F` fullscreen, `Escape` closes a detail, `/` focuses search), `Sidebar.qml` (routes with icons, the three footer rows), `HomeRoute.qml` (`StatusSentence`, `InstrumentStrip` with the mode `SegmentedControl`, input name and rate, `Waveform` fed by `audio.level`, the two primary actions Start dictation and New meeting), `TodayList.qml` (the plain history list with tabular numerals), `RightRail.qml` (engines with the 2 px progress hairline, agent surfaces with their last call), and scaffolds `HistoryRoute`, `MeetingsRoute`, `SettingsRoute` (with the sub-route tabs) each showing the designed empty state from `states-and-hint-sheet.png` and its config keys line; every interactive element carries `Accessible.role` and a stable `Accessible.name` listed in `docs/qa/a11y-names.md` (the existing lint). [paraphrase]
- Every route opens by `DETTIVO_E2E_OPEN=<route>` and `DETTIVO_E2E_ROUTE=<sub>` (the parser exists in `dettivo-core` and the Qt QA environment); the negative text scan of the QA rig runs over each route. [paraphrase]
- Performance: first frame measured by the drive with `QT_QPA` timing and the process's own `firstFrame` log line; idle RSS read from `/proc` after 5 s; recorded in the evidence and asserted on this machine. [inferred]
- Visual: the Home route rendered under `DETTIVO_E2E_OPEN=home` with the seed and the mock daemon state is diffed against crops of `home.png` with the visual-diff tool from the OSD spec on the default and the light theme. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- No new daemon methods; the app is a client of `system.*`, `config.*`, `transcripts.*`, `speech.*`, `dictation.*`, `hotkeys.status`, `insert.target` and the event stream. [paraphrase]
- CLI: `dettivo app` (launch or raise: activates the existing window through its single-instance socket at `$XDG_RUNTIME_DIR/dettivo/app.sock`), `dettivo app open <route>`. [inferred]
- Desktop entry `dettivo.desktop` with the icon from the design system; `QT_QPA_PLATFORM=xcb` under QA. [inferred]
- State file `state.toml` keys documented in `docs/app.md`; nothing of it in `config.toml`. [paraphrase]

## Edge Cases & Constraints

- Daemon not running: the app shows the designed "daemon unavailable" state with the systemd hint and reconnects with backoff; never a blank window. [inferred]
- Second launch: the first window is raised, the second process exits 0. [inferred]
- Theme file replaced atomically by `omarchy theme set` (rename): the watcher re-adds the path and applies within 100 ms. [paraphrase]
- Off Omarchy without a portal: built-in palette, system UI font, monospace for transcripts. [paraphrase]
- Fractional scale change or monitor move: no re-layout glitch, crisp icons. [paraphrase]
- Reduced motion from the portal: transitions cut. [paraphrase]

## Acceptance Criteria

- **R1:** The Xvfb drive opens every FR-U2 route through `DETTIVO_E2E_OPEN` (and every settings sub-route through `DETTIVO_E2E_ROUTE`) on both drivers, asserts the route's accessible title, and the negative text scan passes on each; the a11y lint covers every new element. Errors: an unknown route name exits 2 naming the routes. [paraphrase]
- **R2:** Home matches `home.png`: the status sentence with the real hotkeys and target, the instrument strip with mode segment, input and live waveform from `audio.level`, today's list from the seed with tabular numerals, the right rail's engines and agent surfaces from `engine.state` and `system.health`; the visual diff against the Home crops passes on the default and the light theme in CI. Errors: a missing crop fails by name. [paraphrase]
- **R3:** Live theme: on this machine `omarchy theme set` (or replacing the theme files the same way in a test) re-skins the window with no restart and no flash, and the drive measures the apply time under 100 ms; off Omarchy the portal `color-scheme` switch flips the palette (mocked portal in a unit test). Errors: an unreadable theme file keeps the previous palette and logs once. [paraphrase]
- **R4:** Startup and footprint on this machine: first frame under 300 ms and idle RSS under 60 MB after 5 s, both recorded in the evidence by the drive; the Dettivo style assertion fails the process if a control renders with another style (unit test with a planted control). Errors: as stated. [paraphrase]
- **R5:** The daemon client: reconnects with backoff when the daemon restarts (integration test against a real daemon: kill and restart, the events resume), the "daemon unavailable" state shows and clears, a second launch raises the first window, and `dettivo app open history` routes it. Errors: as stated. [paraphrase]
- **R6:** `docs/app.md` describes routes, QA variables, the state file, the desktop entry and the theme sources; the `Dettivo` module README lists the new components with their tests (Qt Quick Tests for the sidebar, instrument strip, today list and right rail states); an ADR records the host architecture (T2) and the single-instance socket. Errors: as stated. [paraphrase]

## Boundaries

- No onboarding screens (S-18), no settings editors (S-19), no history detail, search, re-run or export (S-20), no meetings routes beyond the scaffold (S-27), no Omarchy plugin (S-21). [paraphrase]
- No keyboard hint sheet content beyond the three conventions (S-22 fills it). [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- The app is the first surface most people will judge, and Omarchy's own apps set the bar: one QML module, a thin host, theme from the shell's files, fast and small. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** the Home route is the approved baseline on the shell's tokens, live with the theme. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** every route is driveable by name under Xvfb from the first commit. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
