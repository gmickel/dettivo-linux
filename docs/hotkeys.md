# Hotkeys

Holding a key dictates. On Omarchy the keys are the ones Voxtype used, so nothing has to be relearned: hold `F9` to talk, `Super+Ctrl+X` to toggle a longer take, `Super+Ctrl+Escape` to drop it, `Super+Ctrl+Shift+X` to insert the last transcript again. Every desktop reaches the same four actions through one of three paths, and every path captures the window you were in when the key went down, so the text lands there and nowhere else (ADR 0016).

| Path | Where | How |
|---|---|---|
| Compositor bindings | Hyprland, Sway, Niri (Omarchy's path) | `dettivo setup <compositor>` writes a snippet that runs the `dettivo` command; the daemon needs nothing. |
| Portal GlobalShortcuts | GNOME, KDE, any desktop whose portal implements `org.freedesktop.portal.GlobalShortcuts` | The daemon binds the four actions once and turns `Activated` and `Deactivated` into press and release. |
| evdev | A desktop with neither | The daemon reads the keyboard devices itself; off by default, needs the `input` group. |

## Omarchy in one command

```
dettivo setup omarchy            # the socket, the snippet below, the bar plugin, both reloads (docs/omarchy.md)
dettivo setup hyprland           # writes ~/.config/hypr/dettivo.lua with your [hotkeys] chords
dettivo setup hyprland --stdout  # print it instead
dettivo setup hyprland --check   # is it loaded from hyprland.lua?
```

On Omarchy, setup writes the snippet, appends its `dofile` include to `~/.config/hypr/hyprland.lua` after the existing configuration, and reloads Hyprland. Repeating setup keeps the existing include and preserves unrelated settings. A failed write, reload or compositor configuration check reports the problem and asks you to retry setup; onboarding stays on the shortcut step until activation succeeds.

Omarchy binds `F9` and `Super+Ctrl+X` to Voxtype when it is installed; the snippet starts with `hl.unbind` for both chords, so loading it after Omarchy's defaults hands the keys to Dettivo. A classic `hyprland.conf` gets `dettivo.conf` with `bind` and `bindr` lines and a `source =` include instead; `dettivo setup hyprland-lua` and `hyprland-conf` pick a flavour by hand.

Hold to talk is a press binding that runs `dettivo --quiet dictation start` and a release binding that runs `dettivo --quiet dictation stop`. Hyprland and Sway fire the release binding when the key of the chord comes up, so release the modifiers last. A release that arrives with no session running (after a compositor restart, say) exits quietly with status 1 and nothing else happens.

## Sway and Niri

```
dettivo setup sway               # ~/.config/sway/dettivo, add: include ~/.config/sway/dettivo
dettivo setup niri               # ~/.config/niri/dettivo.kdl, add: include "dettivo.kdl"
```

Sway gets `bindsym` and `bindsym --release` pairs. Niri has no release bindings, so its snippet carries toggle, cancel and re-insert and says so in a comment; the toggle chord is the hold there, and the hold chord still works through the portal backend on a desktop that has one.

## The chord notation

`[hotkeys]` names chords the way Hyprland's classic configuration does: the modifiers, a comma, the key. `SUPER + CTRL + X` (the Lua spelling) is accepted too, and the snippet generators translate to each compositor's own spelling.

| Config | Hyprland conf | Hyprland Lua | Sway | Niri | Portal trigger |
|---|---|---|---|---|---|
| `F9` | `, F9` | `F9` | `F9` | `F9` | `F9` |
| `SUPER CTRL, X` | `SUPER CTRL, X` | `SUPER + CTRL + X` | `Mod4+Ctrl+x` | `Mod+Ctrl+X` | `LOGO+CTRL+x` |
| `SUPER CTRL, Escape` | `SUPER CTRL, Escape` | `SUPER + CTRL + ESCAPE` | `Mod4+Ctrl+Escape` | `Mod+Ctrl+Escape` | `LOGO+CTRL+Escape` |

Modifiers are `SUPER` (also `LOGO`, `WIN`, `MOD4`), `CTRL`, `ALT` and `SHIFT`. Keys are letters, digits, `F1` to `F12`, and the xkb names of the rest (`Escape`, `Return`, `space`, `Tab`, `BackSpace`, `Delete`, `Insert`, `Home`, `End`, `Prior`, `Next`, the arrows, `Print`, `Pause`, `Menu`, `comma`, `period`, `minus`, `equal`, `slash`, `backslash`, `semicolon`, `apostrophe`, `grave`, `bracketleft`, `bracketright`, the `XF86Audio*` keys); `esc`, `enter`, `pageup`, `pagedown` and `del` are understood as well. A chord that does not parse is refused with the key and the notation named: `hotkeys.hold: cannot parse chord "HYPER, X": unknown modifier HYPER; the notation is ...`.

## What a press does

A press on the hold chord starts a session exactly as `dettivo dictation start` does, and the release stops it; the toggle chord starts when idle and stops when a session runs; cancel and re-insert map one to one. A press while a session runs is ignored with one line in the journal (`hotkey press ignored: a session is active`), never a second session, so a portal binding and a compositor snippet that are both active do not double start.

Before the microphone opens, every start path records the focused window through the same probe `insert.target` uses (Hyprland IPC, X11, the QA mock). The session carries that window as its target, its identity included: `dictation.status` reports it as `target` (`app_id`, `pid`), and when the take is transcribed the insertion is guarded with it, so another window of the same process does not pass. If the focus moved to another window meanwhile, the insertion is refused with `CONFLICT` and the transcript is kept; if the probe could not name a window when the key went down, the take goes to the clipboard (`copied_to_clipboard`, reason `origin_unverified`) rather than into whatever is focused later (`dettivo dictation reinsert-last` hands it over again, unguarded); `dictation.stop` reports the outcome in its `insertion` block. A client that captured the window itself passes it: `dettivo dictation start --expected-target-bundle-id <app id> --expected-target-pid <pid>` (`dictation.start` fields `expected_target_bundle_id` and `expected_target_pid`, `docs/api/linux-deltas.md`).

## Inside the daemon: portal and evdev

`[hotkeys] backend = "auto"` uses the portal's GlobalShortcuts when the desktop portal answers and otherwise nothing, because compositor bindings need no daemon side. `portal` and `evdev` pin a backend; `none` turns both off. The daemon starts the backend on its own thread after the socket is up, restarts it when `[hotkeys]` changes on disk, and reports it:

```
dettivo hotkeys status
backend   portal (requested auto)
portal    available
evdev     unavailable: cannot open 12 input device(s): reading /dev/input/event* needs the `input` group: run `sudo usermod -aG input $USER` and log in again
bound     push_to_talk, toggle, cancel, reinsert_last
```

The portal backend creates one session, binds the four actions with the chords as preferred triggers, and delivers `Activated` and `Deactivated` as press and release. A desktop without the interface (Hyprland's own portal answers `NotAllowed` for a caller without an app id, which a systemd service is), a denied request or a dialog the user dismissed is reported once with its reason in `hotkeys.status` and `dettivo doctor`; nothing is retried in a loop and the daemon keeps serving. `dettivo doctor` also warns when the portal is bound and the compositor snippet is loaded at the same time.

The evdev backend opens the configured `[hotkeys] evdev_devices` (or every keyboard under `/dev/input` when the list is empty), matches the chords with exact modifier sets, and delivers presses and releases the way the compositor would. Reading input devices needs the `input` group; every refusal says so, `dettivo doctor` repeats it, and the daemon still starts.

## Media and sounds

With `[hotkeys] pause_media = true`, every MPRIS player that is `Playing` when capture starts is paused and played again when the session ends, is cancelled or fails. A player you paused yourself stays paused; one that quit meanwhile is skipped. With `[hotkeys] sounds = true`, a short cue plays on start, stop and error through PipeWire's `pw-play`; without `pw-play` on `PATH` the cues stay silent and the journal says so once per cue. Cues never play while a meeting is being captured.

## From the app

The first-run Keys step ([docs/app.md](app.md)) shows the snippet the command above would write and writes it on Continue through two daemon methods that call the same generator: `hotkeys.snippet { compositor? }` answers the text, the include line, the path and the notes; `hotkeys.setup { compositor?, write }` writes the file (a file that already holds the same text is left alone) and reports what `--check` reports: `written`, `sourced`, the include line and the main configuration's path. Without `compositor` both take the session's; a desktop the snippets do not cover answers `INVALID_PARAMS` naming the supported ones. `hotkeys.status.last_press_at` records when a key last reached the daemon through the portal or evdev backend, which is how the step confirms a press live off the compositor path ([docs/api/linux-deltas.md](api/linux-deltas.md)).

## Seeing what runs

```
dettivo hotkeys status           # backend, portal and evdev availability with reasons, bound actions
dettivo doctor                   # the same lines plus: snippet   hyprland: sourced (~/.config/hypr/dettivo.lua)
dettivo status capabilities      # hotkeys.backend and hotkeys.available
```

## Proving it

`dettivo-qa drive hotkeys_hyprland` loads the snippet `dettivo setup hyprland` renders into the running Hyprland with the scenario's own chords and a `dettivo` pointed at the scenario's daemon, presses the chords through a virtual keyboard on `/dev/uinput` so the compositor's own bindings fire, and checks the event stream: hold gives `recording, transcribing, inserting, idle`, toggle the same, cancel `recording, cancelled, idle`. The scenario is skipped with the reason when `hyprctl`, a Hyprland session with the Lua configuration, or a writable `/dev/uinput` is missing. The daemon's tests run the portal backend against a mock `org.freedesktop.portal.GlobalShortcuts` and mock MPRIS players on a private `dbus-daemon` (`crates/dettivod/tests/hotkeys.rs`, `crates/dettivo-hotkeys/tests/`), and the CLI's tests hold the snippets to checked-in goldens (`crates/dettivo-cli/tests/goldens/`).

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[hotkeys] backend` | `"auto"` | `auto`, `portal`, `evdev` or `none`. |
| `[hotkeys] hold` | `"F9"` | Push to talk: press starts, release stops. |
| `[hotkeys] toggle` | `"SUPER CTRL, X"` | One press starts, the next stops. |
| `[hotkeys] cancel` | `"SUPER CTRL, Escape"` | Drops the session. |
| `[hotkeys] reinsert` | `"SUPER CTRL SHIFT, X"` | Inserts the last transcript again (`Super+Ctrl+R` is Omarchy's reminder key). |
| `[hotkeys] pause_media` | `false` | Pause playing MPRIS players while capturing. |
| `[hotkeys] sounds` | `false` | Start, stop and error cues. |
| `[hotkeys] evdev_devices` | `[]` | Devices for the evdev backend; empty means every keyboard. |

Every key is in [docs/config.md](config.md); a change to the chords is picked up by the daemon's backend on reload and by the snippet on the next `dettivo setup`.
