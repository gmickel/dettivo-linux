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

Hold to talk is a press binding that runs `dettivo --quiet dictation press` and a release binding that runs `dettivo --quiet dictation release`. Both feed the daemon's hold key (below), so a release the compositor drops or a key that repeats does not leave a take recording. Hyprland and Sway fire the release binding when the key of the chord comes up, so release the modifiers last. Both verbs are idempotent: an edge that means nothing in the key's state answers `action: none` and exits 0. A snippet an earlier `dettivo setup` wrote runs `dictation start` and `stop`, which keep working with their old contract; run `dettivo setup` again to get the new verbs.

## Sway and Niri

```
dettivo setup sway               # ~/.config/sway/dettivo, add: include ~/.config/sway/dettivo
dettivo setup niri               # ~/.config/niri/dettivo.kdl, add: include "dettivo.kdl"
```

Sway gets `bindsym --no-repeat` and `bindsym --release` pairs; `--no-repeat` keeps a held key from repeating the press, which the hold key would read as a lost release a second in. Niri has no release bindings, so its snippet carries toggle, cancel and re-insert and says so in a comment; the toggle chord is the hold there, and the hold chord still works through the portal backend on a desktop that has one.

## The chord notation

`[hotkeys]` names chords the way Hyprland's classic configuration does: the modifiers, a comma, the key. `SUPER + CTRL + X` (the Lua spelling) is accepted too, and the snippet generators translate to each compositor's own spelling.

| Config | Hyprland conf | Hyprland Lua | Sway | Niri | Portal trigger |
|---|---|---|---|---|---|
| `F9` | `, F9` | `F9` | `F9` | `F9` | `F9` |
| `SUPER CTRL, X` | `SUPER CTRL, X` | `SUPER + CTRL + X` | `Mod4+Ctrl+x` | `Mod+Ctrl+X` | `LOGO+CTRL+x` |
| `SUPER CTRL, Escape` | `SUPER CTRL, Escape` | `SUPER + CTRL + ESCAPE` | `Mod4+Ctrl+Escape` | `Mod+Ctrl+Escape` | `LOGO+CTRL+Escape` |

Modifiers are `SUPER` (also `LOGO`, `WIN`, `MOD4`), `CTRL`, `ALT` and `SHIFT`. Keys are letters, digits, `F1` to `F12`, and the xkb names of the rest (`Escape`, `Return`, `space`, `Tab`, `BackSpace`, `Delete`, `Insert`, `Home`, `End`, `Prior`, `Next`, the arrows, `Print`, `Pause`, `Menu`, `comma`, `period`, `minus`, `equal`, `slash`, `backslash`, `semicolon`, `apostrophe`, `grave`, `bracketleft`, `bracketright`, the `XF86Audio*` keys); `esc`, `enter`, `pageup`, `pagedown` and `del` are understood as well. A chord that does not parse is refused with the key and the notation named: `hotkeys.hold: cannot parse chord "HYPER, X": unknown modifier HYPER; the notation is ...`.

## What a press does

The hold chord goes through one state machine in the daemon, whichever path delivers it: the compositor binding's `dettivo dictation press` and `release`, the portal's `Activated` and `Deactivated`, or evdev.

| The key is | A press | A release |
|---|---|---|
| idle | starts a take; the key is held | nothing |
| held, less than 1 s since the press | nothing: the key is repeating | under 300 ms, a tap: the take is dropped with reason `tap` and no stop cue, and the key waits 300 ms for a second tap; otherwise the take is finished |
| held, 1 s or more | finishes the take: its release was lost | finishes the take |
| held after a second tap | as held | locks: the take keeps recording |
| locked | finishes the take | nothing |
| tapped, within 300 ms | starts the take the next release locks | nothing |

So a release the compositor drops costs one more press, never the 300 s `[dictation] max_duration_seconds` limit, and double-tapping the hold key records hands-free until the next press. While a take is locked the pill shows `locked · press to finish`. Every press and release counts from when it reached the daemon, so one that waited behind a first start loading the engine, or behind `pause_media`, is timed as you pressed it: the wait never turns a repeat into a lost release or a hold into a tap. A take that ended another way (cancel, toggle, the maximum duration) returns the key to idle, so the next press starts a new one.

The toggle chord starts when idle and stops when a session runs; cancel and re-insert map one to one. A hold press while another session runs is ignored with one line in the journal (`hotkey press ignored: a session is active`), never a second session, so a portal binding and a compositor snippet that are both active do not double start.

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

The portal backend first gives the daemon an identity. A host process has no app id unless it registers one, and `xdg-desktop-portal` refuses it a session (`NotAllowed: An app id is required`), which is what a systemd service used to get. The daemon makes sure the desktop entry `dettivo.desktop` exists (the package ships it in `/usr/share/applications`; a source build gets a minimal one under `~/.local/share/applications`, which the daemon removes once the package's entry is installed so it never hides it; an entry you wrote yourself stays), then registers the app id `dettivo` through `org.freedesktop.host.portal.Registry` on the connection that opens the session. A portal older than the registry takes the id from the systemd unit instead. `dettivo hotkeys status` and `dettivo doctor` say which:

```
identity  app id dettivo, registered with the portal
```

When the portal still refuses, the reason names the fix: restart the portal once the desktop entry exists (`systemctl --user restart xdg-desktop-portal`), and start the daemon directly rather than through a setgid wrapper such as `newgrp`, which hides its `/proc` entry from the portal.

The backend then creates one session, binds the four actions with the chords as preferred triggers, and delivers `Activated` and `Deactivated` as press and release. Each shortcut id carries its chord (`dictation-hold-F9`, `dictation-toggle-SUPER_CTRL_X`), because GNOME and KDE keep the first trigger they saw for an id. A changed `[hotkeys] hold` therefore binds a new id when the daemon reloads its configuration, and closing the old session drops the old id. Hyprland's portal accepts the shortcuts but assigns no keys, since Hyprland binds keys in its own configuration; the daemon reports that as `bound     by the compositor`, and the snippet stays the path. A desktop without the interface, a denied request or a dialog the user dismissed is reported once with its reason in `hotkeys.status` and `dettivo doctor`; nothing is retried in a loop and the daemon keeps serving.

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

`dettivo-qa drive hotkeys_hyprland` loads the snippet `dettivo setup hyprland` renders into the running Hyprland with the scenario's own chords and a `dettivo` pointed at the scenario's daemon, presses the chords through a virtual keyboard on `/dev/uinput` so the compositor's own bindings fire, and checks the event stream: hold gives `recording, transcribing, inserting, idle`, toggle the same, cancel `recording, cancelled, idle`, a double tap `recording, cancelled, idle, recording, recording` (the second `recording` is the lock) and then the finishing press, and a lost release (two `dettivo dictation press` calls a second apart) `recording, transcribing, inserting, idle`. `crates/dettivod/tests/hold.rs` drives the same edges through the verbs without a compositor, and the mock portal refuses a session to a connection that registered no app id, so the portal tests prove the registration. The scenario is skipped with the reason when `hyprctl`, a Hyprland session with the Lua configuration, or a writable `/dev/uinput` is missing. The daemon's tests run the portal backend against a mock `org.freedesktop.portal.GlobalShortcuts` and mock MPRIS players on a private `dbus-daemon` (`crates/dettivod/tests/hotkeys.rs`, `crates/dettivo-hotkeys/tests/`), and the CLI's tests hold the snippets to checked-in goldens (`crates/dettivo-cli/tests/goldens/`).

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[hotkeys] backend` | `"auto"` | `auto`, `portal`, `evdev` or `none`. |
| `[hotkeys] hold` | `"F9"` | Push to talk: press starts, release stops; a double tap locks (see above). |
| `[hotkeys] toggle` | `"SUPER CTRL, X"` | One press starts, the next stops. |
| `[hotkeys] cancel` | `"SUPER CTRL, Escape"` | Drops the session. |
| `[hotkeys] reinsert` | `"SUPER CTRL SHIFT, X"` | Inserts the last transcript again (`Super+Ctrl+R` is Omarchy's reminder key). |
| `[hotkeys] pause_media` | `false` | Pause playing MPRIS players while capturing. |
| `[hotkeys] sounds` | `false` | Start, stop and error cues. |
| `[hotkeys] evdev_devices` | `[]` | Devices for the evdev backend; empty means every keyboard. |

Every key is in [docs/config.md](config.md); a change to the chords is picked up by the daemon's backend on reload and by the snippet on the next `dettivo setup`.
