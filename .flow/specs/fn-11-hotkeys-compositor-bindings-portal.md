# Hotkeys: compositor bindings, portal GlobalShortcuts, evdev and MPRIS

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-09 row: "`hotkeys-compositor-bindings-portal-globalshortcuts-mpris` | phase 1 | depends on S-07 | FR-H2 to H6 | Thor scenario with `hyprctl dispatch sendshortcut`; GNOME VM portal scenario; snippet goldens"
> masterplan FR-H2: "`dettivo setup hyprland` (and `sway`, `niri`) writes a bindings snippet implementing hold-to-talk (press starts, release stops) and toggle, defaulting to Omarchy's `Super+Ctrl+X` and `F9`, and prints the include line without editing the user's main config. Niri lacks release bindings, so its snippet offers toggle only and says so."
> masterplan FR-H3: "A portal GlobalShortcuts backend registers the same actions where the portal is implemented, with activated and deactivated signals mapped to press and release."
> masterplan FR-H4: "An optional evdev backend exists for environments without either path, off by default, documented with its group requirement."
> masterplan FR-H5: "MPRIS players are paused on capture start and resumed on stop when the setting is enabled."
> masterplan FR-H6: "Feedback sounds for start, stop and error are optional and off in meetings."
> masterplan T10: "Portals through `ashpd`, libei through `reis` ... Both crates are maintained and cover GlobalShortcuts, RemoteDesktop and Settings; GNOME and KDE support arrives without compositor-specific code."
> masterplan milestone: "First dogfood is S-12 green on Thor: hold F9 in ghostty and text lands."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Holding a key dictates. This spec gives every desktop a way to reach the dictation verbs from S-07: on Hyprland, Sway and Niri a generated bindings snippet drives `dettivo dictation` through the CLI (press starts, release stops, a second key toggles), on GNOME and KDE the portal GlobalShortcuts backend inside the daemon registers the same actions, and an off-by-default evdev backend covers the rest. The daemon captures the focused target at press time through S-08's probe so the insertion guards see the window the user was in, pauses MPRIS players during capture when asked, and plays optional feedback sounds. Omarchy's defaults, `Super+Ctrl+X` for hold-to-talk and `F9` for toggle, are the defaults everywhere. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `dettivo-hotkeys` (depends on `dettivo-proto`, `dettivo-core`): an `Action` enum (`PushToTalk`, `Toggle`, `Cancel`, `ReinsertLast`) and a `HotkeyBackend` trait with `start(handler) -> Result<Registration>` and `stop()`; the handler receives `Press(action)` and `Release(action)`. Backends: `portal` (GlobalShortcuts via `ashpd`: `CreateSession`, `BindShortcuts` with the four actions and their preferred triggers, `Activated`/`Deactivated` signals mapped to press and release; a denied or absent portal reports `Availability::Unavailable(reason)`), `evdev` (reads `/dev/input/event*` through the `evdev` crate, matches key chords from config, off by default, reports the `input` group requirement when devices cannot be opened). Compositor bindings are not a backend: they call the CLI. [paraphrase]
- Daemon wiring: `[hotkeys] backend = "auto" | "portal" | "evdev" | "none"`; `auto` uses the portal when `org.freedesktop.portal.GlobalShortcuts` answers and otherwise nothing (compositor bindings need no daemon side). Press on `PushToTalk` calls the same path as `dictation.start`, release calls `dictation.stop`; `Toggle` starts or stops; `Cancel` and `ReinsertLast` map 1:1. A press while a session runs is ignored with a log line, never a second session. [paraphrase]
- Target at press time: every start path (CLI, portal, evdev) captures `insert.target` before the microphone opens and stores it on the session; S-07's pipeline passes it to the inserter as the expected target so S-08's guards compare the window at insertion time with the window at press time. `dictation.start` gains the optional Linux fields `expected_target_bundle_id` and `expected_target_pid` for callers that captured the target themselves. [inferred]
- `dettivo setup <compositor>`: `hyprland` writes `$XDG_CONFIG_HOME/hypr/dettivo.conf` with `bind`/`bindr` pairs (`bindr` is the release binding) for `Super+Ctrl+X` hold and `F9` toggle, plus `Super+Ctrl+Escape` cancel and `Super+Ctrl+R` re-insert, and prints `source = ~/.config/hypr/dettivo.conf`; `sway` writes `$XDG_CONFIG_HOME/sway/dettivo` with `bindsym` and `bindsym --release`; `niri` writes a `binds { }` block for toggle, cancel and re-insert only and prints why hold-to-talk is absent. Keys come from `[hotkeys] hold`, `toggle`, `cancel`, `reinsert` in config. The command never edits the user's main config; `--stdout` prints the snippet instead of writing; `--check` reports whether the include line is present. Omarchy's own `Super+Ctrl+X` binding conflict is named in the printed notes. [paraphrase]
- MPRIS: on capture start, when `[hotkeys] pause_media = true`, the daemon lists `org.mpris.MediaPlayer2.*` names on the session bus (`zbus`), calls `Pause` on every player whose `PlaybackStatus` is `Playing`, remembers them, and calls `Play` on those players when capture stops or is cancelled. A player that vanished meanwhile is skipped. Off in meetings (S-23 keeps the flag). [paraphrase]
- Sounds: `[hotkeys] sounds = false`; when true, `start`, `stop` and `error` play short bundled OGG files through PipeWire (`pw-play` when present, else the `dettivo-audio` playback stream), never in meetings. [paraphrase]
- Doctor: a `hotkeys` row names the active backend, the portal's availability with reason, the evdev group requirement when selected, and whether the compositor snippet is sourced (`setup --check`). [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dictation.start` accepts the Linux additions `expected_target_bundle_id` and `expected_target_pid` (optional), recorded in `docs/api/linux-deltas.md`; without them the daemon probes the target itself. `dictation.status` reports the captured `target` (app id, pid) while a session runs. [inferred]
- `system.capabilities.hotkeys` names the active backend and lists the available ones; `system.capabilities.hotkey_backend` mirrors the contract's key naming for the macOS field where one exists. [inferred]
- CLI: `dettivo setup hyprland|sway|niri [--stdout] [--check]`, `dettivo hotkeys status` (backend, portal availability, bound actions), and `dettivo dictation start --expected-target-pid <pid> --expected-target-bundle-id <id>`. Exit codes follow the contract's table. [paraphrase]
- Config (`docs/config.md`): `[hotkeys] backend`, `hold = "SUPER CTRL, X"`, `toggle = "F9"`, `cancel = "SUPER CTRL, Escape"`, `reinsert = "SUPER CTRL, R"`, `pause_media = false`, `sounds = false`, `evdev_devices = []`; the snippet generators translate the chord notation per compositor. [inferred]
- Events: a `dictation.state` payload already carries the state; this spec adds no topic. [inferred]

## Edge Cases & Constraints

- A release without a press (the snippet's `bindr` fires after a compositor restart): `dictation.stop` answers `NOT_FOUND`, the binding runs `dettivo dictation stop` which exits 1 quietly under `--quiet`; the snippet uses `--quiet`. [inferred]
- Two backends both active (portal plus a compositor snippet): the daemon ignores a start while a session runs, so double presses do not double start; the doctor warns when the portal is bound and the snippet is sourced. [inferred]
- The portal denies the request or the desktop has no GlobalShortcuts implementation (Hyprland today): the backend reports unavailable with the reason and `auto` falls back to nothing without an error at startup. [paraphrase]
- evdev without permission: the backend fails with the `input` group requirement in the message and the doctor repeats it; the daemon still starts. [paraphrase]
- Niri: hold-to-talk is absent and the snippet says so. [paraphrase]
- MPRIS players that were paused by the user before capture are never resumed. [inferred]

## Acceptance Criteria

- **R1:** Snippet goldens: `dettivo setup hyprland --stdout`, `sway --stdout` and `niri --stdout` match checked-in golden files for the default keys and for custom keys from config; `hyprland` and `sway` include press and release bindings, `niri` includes toggle only and a note. Errors: an unknown compositor exits 4 naming the supported ones; an unparseable chord names the key and the notation. [paraphrase]
- **R2:** The Hyprland scenario on the development machine (`dettivo-qa drive hotkeys_hyprland`): with the snippet sourced, `hyprctl dispatch sendshortcut` for the hold chord starts a session and the release stops it, the toggle chord starts and stops, cancel cancels, and the dictation event stream shows every transition; the scenario is skipped with the reason when `hyprctl` is absent. Errors: as stated. [paraphrase]
- **R3:** Portal backend unit and integration tests: a mock `org.freedesktop.portal.GlobalShortcuts` on a private session bus (the QA rig's `dbus-run-session`) answers `BindShortcuts`, and `Activated`/`Deactivated` signals start and stop a session; an absent portal reports unavailable with its reason and the daemon starts. Errors: a denied session is reported once, not retried in a loop. [paraphrase]
- **R4:** Target at press time: a start through the CLI, the portal or evdev records the focused target on the session, `dictation.status` reports it, and the insertion at the end of the session is refused with `CONFLICT` when the focus moved to another window (QA mode with `DETTIVO_MOCK_A11Y` and the mock insert path). Errors: as stated. [inferred]
- **R5:** MPRIS and sounds: with `pause_media = true`, a mock MPRIS player on the private bus is paused on start and resumed on stop and on cancel, an already paused player is left alone; with `sounds = true` the start and stop sounds play through the mock audio path in QA mode and never during a meeting flag. Errors: a vanished player is skipped without failing the session. [paraphrase]
- **R6:** `dettivo doctor` and `dettivo hotkeys status` list the backend, the portal availability and reason, the evdev requirement, and whether the compositor snippet is sourced; the `[hotkeys]` keys are documented in `docs/config.md` and printed by `config print-default`; `docs/hotkeys.md` explains the three paths and the Omarchy defaults. Errors: as stated. [paraphrase]

## Boundaries

- No OSD; S-10 shows the states this spec triggers. [paraphrase]
- No Omarchy plugin or bar widget; S-21 reads the same event stream. [paraphrase]
- No first-run bindings screen; S-18 calls `dettivo setup`. [paraphrase]
- Language slots, `insertRaw`, `insertEnhanced`, `captureNote` and `quickAccess` stay out of v1 (FR-H1). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The first dogfood is "hold F9 in ghostty and text lands"; Omarchy has no GlobalShortcuts portal, so compositor bindings through the CLI are the primary path and the portal serves GNOME and KDE. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the hotkey is the start of every dictation, and the Hyprland scenario proves press, release and cancel end to end. [strategy:Complete speech workflows, proven by drives]
- **Omarchy-native, beautiful by default:** the snippet uses Omarchy's own chords and include convention and never edits the user's config. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
