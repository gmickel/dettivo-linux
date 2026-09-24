# Feature map

Committed user-POV drive knowledge for Dettivo for Linux. How a user reaches each feature, how an agent drives it, and which traps waste a run. Seeded from the design coverage table in `docs/design/README.md` at the repository bootstrap, before any surface can be driven: every entry below is identified, none is proven, and a feature file is written only once a live drive has proven its route.

## Baseline preconditions

- Launch target: the Qt binaries from `build/qt` (`dettivo-app`, `dettivo-osd`, `dettivo-insert-target`) after `just build`, and the daemon `target/debug/dettivod` once the daemon spec lands. Until then every binary answers `--version` only.
- Disposable data: point `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_RUNTIME_DIR` at a per-run temporary directory so a drive never touches the operator's `config.toml`, history database or socket.
- Seed state: QA mode is product code switched by the macOS environment names (`DETTIVO_MOCK_MODE`, `DETTIVO_E2E_OPEN`, `DETTIVO_E2E_SEED`); a drive names the seed it used.
- Display: the Qt binaries run with `QT_QPA_PLATFORM=xcb` and `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` so cua-driver and the in-repo AT-SPI driver can enumerate them; XWayland locally, Xvfb in CI.
- Isolation: two instances can run side by side only with distinct `XDG_RUNTIME_DIR` values, because the daemon socket lives at `$XDG_RUNTIME_DIR/dettivo/dettivo.sock`. A run refuses to drive a daemon it did not start.

## Driving conventions

- Stable handles: accessible roles and names, window titles, CLI invocations. Never pixel coordinates.
- Commands are literal. Copy them as written in the feature file.
- Re-read the accessibility tree after every navigation, click or submit; element indices go stale.

## Proof standards

- Capture the user action and the resulting state, not just the final screen.
- Verify side effects beside what is visible: the socket reply, the history database, the CLI exit code, the text that landed in the target window.
- Real user paths only. Never a test-only endpoint.
- Unreachable: report the attempted route and the unmet precondition. Never verified-via-another-path.

## Feature-entry contract

Each feature file follows the four-H2 contract (Sub-features / How to get to it (user POV) / Driving it / Gotchas) plus the required `**Surface:**` line, as defined by the flow-next features skill. Consumers select by surface + sub-feature IDs.

## Surfaces

### app

The `dettivo-app` window. No feature files yet; the routes below come from the approved baselines and get a file when the surface exists and a drive proves it.

Identified, not yet proven (retry next pass):

| Planned file | Feature | Sub-features | Baseline |
|---|---|---|---|
| `app-home.md` | Home | `home.status`, `home.instrument`, `home.today`, `home.rail` | `home.png`, `home-light-catppuccin-latte.png` |
| `app-history.md` | History list and dictation detail | `history.list`, `history.search`, `history.detail`, `history.audio` | `history.png` |
| `app-meetings-list.md` | Meetings list | `meetings.list`, `meetings.new` | `meetings-list.png` |
| `app-meeting-live.md` | Meeting, live | `meeting.live.controls`, `meeting.live.transcript`, `meeting.live.notes` | `meeting-live.png` |
| `app-meeting-detail.md` | Meeting detail after stop | `meeting.detail.transcript`, `meeting.detail.notes`, `meeting.detail.analysis`, `meeting.detail.export` | `meeting-detail.png` |
| `app-first-run.md` | First run: Keys, Models, Try it | `firstrun.keys`, `firstrun.models`, `firstrun.tryit` | `first-run-1-keys.png`, `first-run-2-models.png`, `first-run-3-try-it.png` |
| `app-settings-models.md` | Settings, Models | `settings.models.select`, `settings.models.table` | `settings-models.png` |
| `app-settings-hotkeys.md` | Settings, Hotkeys | `settings.hotkeys.table`, `settings.hotkeys.snippet`, `settings.hotkeys.toggles` | `settings-hotkeys.png` |
| `app-agents.md` | Agents | `agents.socket`, `agents.rest`, `agents.mcp` | `agents.png` |
| `app-import-and-disclosure.md` | Import dialog and disclosure dialog | `import.dialog`, `disclosure.dialog` | `import-and-disclosure.png` |
| `app-states.md` | Empty, loading and error states; keyboard hint sheet | `states.empty`, `states.loading`, `states.error`, `hints.sheet` | `states-and-hint-sheet.png` |
| `app-design-sheet.md` | Component sheet | `design.sheet` | `design-system.png` |

### osd

The `dettivo-osd` pill, layer-shell on Omarchy and a plain always-on-top window elsewhere.

Identified, not yet proven (retry next pass):

| Planned file | Feature | Sub-features | Baseline |
|---|---|---|---|
| `osd-states.md` | OSD states | `osd.listening`, `osd.transcribing`, `osd.enhancing`, `osd.inserted`, `osd.copied`, `osd.error` | `osd.png`, `icon-and-osd-elsewhere.png` |

### bar

The Omarchy bar widget and its panel, rendered by Quickshell from the installed `Dettivo` QML module.

Identified, not yet proven (retry next pass):

| Planned file | Feature | Sub-features | Baseline |
|---|---|---|---|
| `bar-widget-and-panel.md` | Omarchy bar widget and panel | `bar.glyph`, `bar.panel.state`, `bar.panel.actions`, `bar.panel.recent` | `omarchy-bar-panel.png`, `osd.png` (glyph states) |

### cli

The `dettivo` command and the `dettivo-mcp` server.

No CLI features seeded this pass; the contract spec defines the commands a drive can cite.
