# The Dettivo design system

Import `Dettivo` and every surface you build looks like part of the desktop it runs on. On Omarchy the palette, the control states, the font and the spacing come straight from the active theme's `colors.toml` and `shell.toml` (a theme without a `shell.toml`, or one that names only some tokens, fills the rest from its own roles), and they follow `omarchy theme set` within 100 ms without a restart. Anywhere else the same tokens resolve from the portal colour scheme with a built-in dark and light palette designed to the same roles. The approved look is the Studio sheet under `docs/design/studio/`; this module is that sheet as code (ADR 0010).

## What you get

- **`Theme`**, one singleton with every token: the palette, semantic roles (`roleText`, `roleMutedText`, `roleBorder`, `roleHairline`, `roleAccent`, `roleSelected`, `roleUrgent`, the speaker colours), the type scale (`typeDisplaySize` down to `typeLabelSize`, tabular numerals, the light weight for provisional text), the spacing scale (`space1` to `space8`), control geometry (`controlHeight`, `rowHeight`, `rowPaddingX`) and the shell's per-state fill and border alphas. `Theme.source` says where they came from: `omarchy`, `builtin-dark` or `builtin-light`.
- **`Motion`**, the named durations and easings (enter, exit, reveal, pill, level, shimmer) and `reducedMotion` from the desktop portal. `Motion.duration(ms)` returns zero when the user asks for reduced motion.
- **`Html`**, `Html.escaped(text)` for a daemon fact (a hotkey chord, an app id) that goes into rich text: `<`, `>`, `&` and quotes become entities, so the fact is drawn, never parsed as markup.
- **`DettivoStyle`**, a Qt Quick Controls style that draws buttons, text fields, combo boxes, check boxes, radio buttons, switches, scroll bars, menus, popups, tool tips, list delegates, tab bars and progress bars with the shell's normal, hover, focus, selected and pressed states. `Button { highlighted: true }` is the accent primary action and `Button { danger: true }` the urgent one.
- **Components** every screen shares: `SectionLabel`, `Chip`, `KeyCap`, `StatusDot`, `LevelMeter`, `Waveform`, `SegmentedControl`, `KeyValueRow`, `ListRow` with the selection rail, and `Icon` over the 16 px icon set.
- **`Osd`**, the recording pill ([docs/osd.md](../../../docs/osd.md)): six states plus hidden on the item's `state`, the level, the texts, the hide timers and the accessible names, with `OsdBars` (a C++ scene-graph item) for the live bars. A host binds the properties and owns the window; `dettivo-osd` does that outside Omarchy, and the Omarchy panel plugin will host it there.
- **`BarGlyph` and `BarPanel`**, the Omarchy bar's mark and its 340 px panel ([docs/omarchy.md](../../../docs/omarchy.md), ADR 0030): five glyph states with the meeting timer and a `dimmed` hint, and the panel's states (idle, recording, transcribing, inserted, meeting, unavailable, hint) with its header, mode segment, actions, fact rows, recent list and footer as signals a host wires to the `dettivo` command; `dettivo-bar` renders both for the visual job, `tst_bar.qml` covers them.
- **The app's screens** under `app/` ([docs/app.md](../../../docs/app.md), ADR 0020): `AppWindow` (the window, the page the router names, the three key conventions), `Sidebar` with `SidebarItem` and the three `SidebarFooterRow`s, `PageHeader`, `HomeRoute` with `InstrumentStrip`, `TodayList`, `RightRail` and `RailRow`, `SectionHeading`, `DaemonBanner`, `StateView` (the designed state: sentence, reason, key or button, the urgent square, the loading thread), `HintSheet` (the keyboard hint sheet `?` opens), `StatesPage` (one designed state rendered for the visual job under `DETTIVO_E2E_STATE`), `RouteScaffold` and the scaffolds `HistoryRoute`, `MeetingsRoute`, `SettingsRoute`; first run (ADR 0024) is `OnboardingRoute` with `KeysStep`, `ModelsStep`, `TryItStep`, `BindingRow`, `ModelRow` and `FirstRunFooter` over the host's `FirstRunModel`, covered by `tst_app_first_run.qml`. A screen takes its router, models and config binding as plain object properties, so a host hands in its own and the tests hand in fakes; `tst_app_shell.qml` covers the sidebar, the banner, the empty state and the window's page swap by route; `tst_app_home.qml` the instrument strip, Today with rows and empty, and the rail's states and progress hairline.
- **Icons** as SVG sources on a 16 px grid with a 1.5 px stroke and square caps, recoloured by any token at runtime; `sixbar` is the mark, and `icons/dettivo.svg` installs as the desktop icon.
- **A component sheet**, `dettivo-sheet`, that renders the whole system at 1280 by 1700; `dettivo-sheet --screenshot out.png` writes it headlessly for comparison with `docs/design/studio/baselines/design-system.png`.

## Using it

```qml
import QtQuick
import QtQuick.Controls
import Dettivo

Rectangle {
    color: Theme.roleSurface
    Button { highlighted: true; icon.name: "mic"; text: qsTr("Start dictation") }
}
```

Hosts built with `dettivo_add_host` select the style before the first control exists and refuse to run with any other style. A host of your own does the same with `QQuickStyle::setStyle("DettivoStyle")` before creating its engine, plus `/usr/lib/dettivo/qml` on the import path for installed builds.

## Rules the lints keep

- No component or screen carries a literal colour or pixel size beyond the structural `"transparent"` and `0`; `scripts/lint-qml-tokens.sh` is a heuristic that names the file and property when a hex colour, a named colour or a bare size literal slips in.
- Every icon is on the 16 px grid with one stroke width and the recolour key; `scripts/lint-icons.sh` checks each file.
- Every control and component has an accessible role and name, and every fixed name is listed in `accessible-names.txt`; `scripts/lint-accessible-names.sh` reads that list.
- Every component and screen has a Qt Quick Test under `tests/qml/` covering its states and names; `just test-qt` runs them with the style active.
- Every fixed accessible name is documented in `docs/qa/a11y-names.md`, the table a drive author reads; the same lint checks it.

## Testing against another theme

Point `DETTIVO_OMARCHY_THEME_DIR` at any directory holding a `colors.toml` (and a `shell.toml` when the theme has one) and every binary resolves from it; `DETTIVO_REDUCED_MOTION=1` forces reduced motion. `dettivo app status` and `dettivo osd status` print the `theme` block the backend resolved: `source`, `dir`, `background`, `accent` and `parse_error`. The theme backend tests swap files under a temporary directory to prove the 100 ms re-resolve, resolve the checked-in copies under `qt/fixtures/themes/` (`last-call` is the theme published on the development desktop, with its origin noted in `colors.toml`), and the `app_theme_live` drive swaps the directory's symlink under the running app the way `omarchy theme set` does ([docs/app.md](../../../docs/app.md)).
