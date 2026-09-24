# Settings routes as a config editor

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-19 row: "`settings-routes-as-config-editor` | phase 3 | depends on S-17, S-14 | FR-C1, C3, C4, C6, C7; settings routes including diagnostics and the Agents page; `dettivo config` CLI; `docs/config.md`; baselines `settings-models.png`, `settings-hotkeys.png`, `agents.png` | Drive pack per route; round-trip through daemon config with comments preserved; `config` CLI snapshots; every key documented (lint); visual diff against the settings pattern"
> masterplan FR-C3: "Settings changed in the GUI are written through the daemon so validation lives in one place, and writes preserve the user's comments and ordering (`toml_edit`), so a hand-maintained file stays hand-maintainable."
> masterplan FR-C6: "Every settings route in the GUI shows the config keys it edits and offers "open config.toml", so the file is never a hidden second system."
> masterplan FR-U2: "settings sub-routes (general, hotkeys, models, polish, insertion, meetings, agents, diagnostics)"
> masterplan coverage table: "Settings pattern, Models and Hotkeys routes | Designed, awaiting approval | `settings-models.png`, `settings-hotkeys.png`"; "Settings: General, Polish, Insertion, Meetings, Diagnostics | Follow the pattern; built to it"; "Agents page | Designed, awaiting approval | `agents.png`"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Settings is the config file with a face. The app's eight settings sub-routes (general, hotkeys, models, polish, insertion, meetings, agents, diagnostics) become editors over `config.toml` through the daemon's `config.*` methods, so validation lives in one place and comments survive; every route shows the keys it edits and offers "open config.toml"; Models and Hotkeys are built to their baselines and the other routes follow the settings pattern; Agents is the designed page for MCP and REST setup; Diagnostics is `dettivo doctor` on screen. A drive pack per route round-trips every editable key and the key-coverage lint proves every documented key has an editor or a documented reason not to. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Host: `ConfigBinding` (from the app shell) grows a `SettingsModel` per route with typed fields backed by `config.get` (value and source) and `config.set|unset` through the daemon (validation errors come back as the daemon's messages and show inline), change subscription through the daemon's config reload event so the CLI and the GUI converge; a `KeyRegistry` generated from the schema (`dettivo config keys --json`, a new CLI verb over the schema's key list with types, defaults and docs) drives the "keys this route edits" line and the coverage lint. [inferred]
- QML under `qt/qml/Dettivo/app/settings/`: `SettingsRoute.qml` with the section tabs and the pattern (`SettingsPage`, `SettingsGroup`, `SettingRow` with the key name, the control, the source badge for environment overrides, and inline validation), `GeneralSection`, `HotkeysSection` (the chords as editable key caps with `dettivo setup` preview and the write button, the backend picker, MPRIS and sounds; built to `settings-hotkeys.png`), `ModelsSection` (providers and models with readiness, download and delete with progress, selection, the LLM model when the engine spec has landed; built to `settings-models.png`), `PolishSection` (transforms, default preset and style, custom rules, app profiles table, providers and the trust list), `InsertionSection` (backend pin, paste keys table, terminal app ids, restore clipboard, undo window), `MeetingsSection` (keep audio, checkpoint interval, artifacts, the disclosure state with the copy action), `AgentsSection` (MCP host configuration through `dettivo mcp config` per host with the write action, REST enable and port and the token source, hardened toggle; built to `agents.png`), `DiagnosticsSection` (the doctor report on screen with copy). "Open config.toml" launches `xdg-open` on `config.path`. [paraphrase]
- List and table values (paste keys, app profiles, trusted endpoints, self app ids) edit through `config.set` with list and table encoding the daemon already accepts. [paraphrase]
- Drives: `settings_roundtrip` per route: open the route, change every editable key through the control, assert `config.get` reports the new value with source `file`, assert the file's comments survived (a comment planted in the profile's config), reset with `unset`; `settings_visual` diffs Models, Hotkeys and Agents against their baselines. The key-coverage lint (`scripts/lint-settings-keys.sh`) compares the registry against the routes' declared keys and fails on an undeclared key. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `config.get|set|unset|path|validate` (existing); `config.keys` (Linux addition: every key with type, default, doc and section) with a fixture; `config.changed` event topic (Linux addition) so open editors refresh when the file changes. [inferred]
- CLI: `dettivo config keys [--json]`, `dettivo app open settings.<section>`. [inferred]
- QA: `DETTIVO_E2E_ROUTE=<section>` (existing). [paraphrase]

## Edge Cases & Constraints

- A key set by environment: the row shows the source badge and disables the control with the variable's name. [paraphrase]
- Invalid value: the daemon's validation message shows inline; the file is untouched. [paraphrase]
- The file edited by hand while a route is open: the route refreshes from `config.changed`, unsaved edits are kept with a notice. [inferred]
- A route for a spec that has not landed (meetings before S-23): the keys exist in the schema already; the section edits them. [inferred]

## Acceptance Criteria

- **R1:** Every editable key of every section round-trips through the control and `config.get` with source `file`, a planted comment survives, `unset` restores the default (the `settings_roundtrip` drive on both drivers under Xvfb); the coverage lint fails on an undeclared key and passes on the branch. Errors: an invalid value shows the daemon's message inline. [paraphrase]
- **R2:** Models, Hotkeys and Agents match their baselines region by region on the default and light themes in CI; the other sections follow the pattern (visual entries for one of them). Errors: a missing crop fails by name. [paraphrase]
- **R3:** Every route shows the keys it edits and "open config.toml" launches the file; environment overrides show the source badge and the disabled control. Errors: as stated. [paraphrase]
- **R4:** Agents: the MCP host entries render and write through the same code as `dettivo mcp config`, REST enable and port and the token source show, Diagnostics renders the doctor report with copy. Errors: as stated. [paraphrase]
- **R5:** `config.keys` and `config.changed` have fixtures and pass; `dettivo config keys --json` matches the schema; an editor refreshes when the file changes. Errors: as stated. [paraphrase]
- **R6:** `docs/app.md` gains the settings section, `docs/config.md` links each key to its route, the deltas are registered, an ADR records the registry and the write path. Errors: as stated. [paraphrase]

## Boundaries

- No first-run flow (S-18), no history detail (S-20). [paraphrase]
- No settings for specs whose keys do not exist yet beyond what the schema has. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- The file stays the truth; the GUI is a convenience that never hides a second system, which is what dotfile users on Omarchy expect. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** editors on the settings pattern, the file always one click away. [strategy:Omarchy-native, beautiful by default]
- **Contract parity and agent surfaces:** the Agents page writes the same MCP and REST configuration the CLI does. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
