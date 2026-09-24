# Dettivo in the Omarchy bar

On Omarchy, Dettivo lives in the bar. A 16 px mark in the bar's right section shows what the daemon is doing: idle, listening with the bars following your voice, transcribing, a meeting with its elapsed time, or an error. It gets the bar's active underline while a take runs, and a click opens the panel: the state, the mode, Dictate and the meeting action, which engine, language model and insertion target are in play, your last three dictations, and Open Dettivo with its shortcut. The recording pill is drawn by the shell itself, so Omarchy runs no extra Dettivo window. The approved design is `docs/design/studio/baselines/omarchy-bar-panel.png`, and every render is checked against it (ADR 0030).

## Install

```
yay -S dettivo-bin          # or a build of this repository
dettivo setup omarchy       # the socket, the bindings, the plugin, both reloads
```

`dettivo setup omarchy` enables `dettivod.socket`, writes `~/.config/hypr/dettivo.lua` with your `[hotkeys]` chords, includes it in the Lua configuration and verifies the compositor reload ([docs/hotkeys.md](hotkeys.md)), copies the plugin from `/usr/share/dettivo/omarchy` into `~/.config/omarchy/plugins/gmickel.dettivo/`, enables it in the bar's right section, then runs `hyprctl reload` and asks the shell to rescan its plugins. Each step prints its state; `--check` reports them as JSON afterwards, `--stdout` prints what the command would write and run, `--no-plugin` leaves the plugin alone, and `--git https://github.com/gmickel/omarchy-dettivo.git` adds the plugin from the mirror repository through `omarchy plugin add` instead of the local copy, so `omarchy plugin update` keeps it current.

```
dettivo setup omarchy --check
dettivo doctor              # the omarchy line: shell, plugin version, whether the panel holds the bus
```

On a plain Hyprland without the `omarchy` command the plugin step is skipped with the reason and `dettivo-osd.service` keeps the pill.

The shipped plugin includes its `DettivoState` singleton registration. The panel loads both packaged QML modules by directory, so the shell needs no `QML_IMPORT_PATH` override. A shell rescan discovers plugins; an already loaded plugin can retain cached QML until the shell is restarted. Restart the shell at a convenient time after updating its code.

## The bar widget

The idle waveform keeps its button visible even though it replaces the text label. The panel uses equal-width Raw, Polish and Enhanced segments and explicit Dettivo-styled action buttons, preserving the theme in a shell with a different global Qt control style.

The mark is `Dettivo.BarGlyph` from the shared module: six bars whose colour is the state (muted while idle, the accent while listening and in a meeting, the accent dimmed while transcribing, the urgent colour on an error) and whose heights follow `audio.level` while `level_meter` is on. A meeting shows its timer beside the mark, counting from the recording's start on the plugin's own clock and re-anchored by every state event and snapshot (ADR 0063); when the recording stops the count leaves the mark, which dims to the transcribing tone until the transcript is built. Middle-click starts a dictation; left-click opens the panel. When the module or the daemon is missing the mark dims and the tooltip says what to install.

## The panel

| Row | What it shows | What it does |
|---|---|---|
| Header | The mark, `Dettivo`, and the state sentence: `Ready.`, `Listening · release to insert`, `Transcribing`, `Inserted into <app>`, `Meeting recording · 00:23:41`; `REC` while a take or a meeting runs | |
| Mode | `Raw`, `Polish`, `Enhanced` | writes `dictation.mode` |
| Actions | `Dictate` and `Start meeting`; `Stop` and `Cancel` while listening; `Stop meeting` during a meeting | Dictation uses `start\|stop\|cancel`; meeting start opens the app's setup/disclosure flow, and observed stop sends the active meeting ID (ADR 0055). |
| Facts | `ENGINE`, `ENHANCED`, `INSERT INTO` | from `speech selection`, `llm providers list`, `insert target` |
| Recent | The last `history_items` dictations with their time and target app | opens History in the app |
| Footer | `Open Dettivo` with `open_shortcut` | `dettivo app open home` |

When the daemon is down the header says `Daemon unavailable.` with `systemctl --user start dettivod.socket` under it and the body hides; the event stream reconnects with the pill's backoff, half a second to eight, and every fact is re-read when the daemon is back. When `dettivo` is not installed, the shared module is missing at `/usr/lib/dettivo/qml`, or the daemon is older than the plugin needs (`omarchy.minDettivo` in the manifest), the panel shows one hint (`Install Dettivo` with `yay -S dettivo-bin`, or `Upgrade Dettivo` with both versions) and nothing throws.

## The pill

With `[omarchy] osd = "panel"` (the default) the plugin's `panel` kind hosts `Dettivo.Osd` on a layer-shell window at the `[osd]` position and holds the session-bus name `dev.dettivo.OmarchyPanel` through `dettivo osd host-panel`; `dettivo-osd` sees the name and exits with a notice, so one pill shows, and `dettivo osd status` reports it. The claim runs only while the shared module is installed and the pill component loads, and the window shows once the claim has answered `claimed`, so a shell without the module leaves the name free and `dettivo-osd` keeps the pill. The pill's completion is the daemon's own: the `dictation.state` transition into `idle` from `inserting` carries the insertion and the first words ([docs/api/linux-deltas.md](api/linux-deltas.md)), and the idle that follows a failure leaves the reason on the pill until it hides itself.

```
$ dettivo osd status
host = "disabled"
notice = "the Omarchy panel plugin hosts the pill on this session (dev.dettivo.OmarchyPanel is on the bus)"
```

`osd = "service"` leaves the name free so `dettivo-osd.service` keeps the pill (restart the service after changing it); `off` shows no pill on Omarchy. The pill's states, texts and timers are the component's own ([docs/osd.md](osd.md)).

## Settings

Everything lives in `[omarchy]` of `config.toml` ([docs/config.md](config.md)); the plugin reads the section through `dettivo config get` and its folder carries no state, so `omarchy plugin update` fast-forwards.

| Key | Default | Meaning |
|---|---|---|
| `glyph` | `"waveform"` | The six-bar mark, or `dot`. |
| `level_meter` | `true` | The bars follow the live level while recording. |
| `osd` | `"panel"` | `panel`, `service` or `off`: who shows the pill. |
| `history_items` | `3` | How many recent dictations the panel lists. |
| `open_shortcut` | `"SUPER SHIFT, D"` | The label beside Open Dettivo. |

The shell's settings sheet for the widget carries `glyph`, `levelMeter` and `osd`; a value changed there is written to the file through `dettivo config set omarchy.*`.

## The plugin folder and the mirror

The plugin is `omarchy/` in this repository: `manifest.json` (schema 1, id `gmickel.dettivo`, kinds `bar-widget` and `panel`, `keepLoaded`, category Developer Tools, the three settings entries, `omarchy.minDettivo`), `BarWidget.qml`, `PanelPopup.qml`, `Panel.qml`, `DettivoState.qml` and the three files that import the module. The package installs it at `/usr/share/dettivo/omarchy`. The repository `gmickel/omarchy-dettivo` is a mirror of that folder for `omarchy plugin add`: every release's `distribute` workflow runs `scripts/omarchy/export-plugin.sh`, which copies the folder in byte for byte and refuses a manifest version that differs from `Cargo.toml`'s, then commits and pushes the mirror as `v<version>` ([docs/RELEASING.md](RELEASING.md#the-changelog-the-docs-and-the-tag), [ADR 0064](adr/0064-a-release-publishes-itself-to-the-aur-and-the-plugin-mirror.md)). `dettivo doctor` names the plugin version the shell has. Nothing in the mirror is edited by hand.

## Proving it

- `just lint` runs `scripts/lint-omarchy-plugin.sh`: the manifest fields the shell needs, `omarchy plugin validate` where the shell is installed, the version against `Cargo.toml`, no symlinks, and every QML file through `qmllint --bare` and `qmlformat` against the shell shim under `qt/fixtures/omarchy-shell/` with the module from the build tree.
- `just test-qt` runs `tst_bar.qml` (the glyph's states and the panel's rows, actions, names and hint states) and `dettivo-omarchy-plugin-test` (`qt/fixtures/omarchy-shell/tests/tst_plugin_hints.qml` and `tst_plugin_events.qml`: the plugin under an unlinked Basic-style Qt host with no Dettivo import-path override, using the shipped singleton registration and visible-button contract, with the install hint against a missing module, the upgrade hint against an old `system.version`, the event stream driving the glyph, the header and the pill with the contract's `dictation.state` completion snapshot, every action as one `dettivo` command, and the bus-name claim in panel mode with the module present only, its window shown once the claim is answered).
- `just qa-visual` renders `panel` (idle, recording, transcribing, meeting, hint) and `bar-glyph` (idle, listening, transcribing, meeting, error) through `dettivo-bar` on every theme and scale against the crops of `omarchy-bar-panel.png` and `osd.png` and the approved renders ([docs/design/baselines.md](design/baselines.md)); `DETTIVO_E2E_BAR_STATE` names the surface and state.
- `just qa-drive omarchy_osd_host` proves the hand-over on any desktop with a session bus: with `osd = "panel"` the claimer holds the name, `dettivo-osd` exits with the notice and `dettivo osd status` reports it; with `osd = "service"` the claimer exits at once and `dettivo-osd` shows the pill. `just qa-drive omarchy_bar` runs where the shell has the plugin enabled: a CLI-started dictation is read back through `dettivo dictation status --json` at every state, the bar is captured with `grim` and the widget's crop is compared with the baseline; anywhere else it reports why it skipped.
