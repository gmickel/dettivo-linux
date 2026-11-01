# 0083. The hold key is a daemon state machine, and the portal gets an app id

Status: Accepted 2026-11-01

## What this gives you

Holding `F9` finishes the take even when the compositor drops the key release: the next press after a second ends it instead of the 300 s `max_duration_seconds` limit. A tap and then a press within 300 ms record hands-free until the next press. On a stock Omarchy install the GlobalShortcuts portal opens a session for the daemon, and a changed `[hotkeys] hold` reaches GNOME and KDE after a reload.

## Situation

- `dettivo doctor` on Omarchy reported `portal session refused: ... NotAllowed: An app id is required`. The daemon runs as a host process from a systemd unit whose name carries no app id, so `xdg-desktop-portal` refuses it a session.
- GNOME and KDE keep the first trigger they saw for a shortcut id and ignore a later `preferred_trigger`. The shortcut ids were the action names (`push_to_talk`), so a rebind could do nothing.
- Hyprland's Lua binds and GNOME can drop a key release, and some compositors repeat a held key as fresh presses. The hold binding ran `dictation start` and `dictation stop`, so a lost release left the take recording and a repeat was a refused second start.
- Hyprland's portal answers a bind with empty triggers, because Hyprland binds keys in its own configuration.

This record amends [0016](0016-hotkeys-compositor-bindings-portal-evdev.md).

## Decision

- **One state machine for the hold key.** `dettivo_hotkeys::hold` holds four states (`idle`, `held`, `locked`, `tapped`) and three timings: `REPEAT_FILTER` 1 s, `DOUBLE_TAP` 300 ms and `MIN_HOLD` 300 ms. A press while held under 1 s is a repeat and does nothing. A press while held for longer finishes the take, because the release was lost. A release under 300 ms is a tap, which drops the take it started. A press within 300 ms of a tap starts a take whose release locks it, and the next press finishes a locked take. The hold counts from when recording began, so a start that loads the engine does not turn a queued repeat into a lost release.
- **The daemon owns the machine.** `HoldKey` in `dettivod` feeds it from every path: `dictation.press` and `dictation.release` (the compositor bindings), the portal's `Activated` and `Deactivated`, and evdev. It owns the take it started. A take that ended or stopped recording any other way (cancel, toggle, the maximum duration) returns the key to idle before the next edge is read. Locking publishes a `dictation.state` from `recording` to `recording` with reason `locked`, and the pill shows `locked · press to finish`.
- **New idempotent verbs; `start` and `stop` keep their contract.** `dettivo setup hyprland` and `sway` write `dettivo --quiet dictation press` and `release`; the Sway press is `bindsym --no-repeat`, because Sway repeats a held binding and the machine would read a repeat after 1 s as a lost release. Existing `start` and `stop` bindings keep working outside the machine. Niri has no release bindings and keeps `toggle`.
- **The portal identity is registered.** Before the portal backend starts, the daemon makes sure a `dettivo.desktop` entry exists, writing a minimal one under `$XDG_DATA_HOME/applications/` when neither that directory nor `$XDG_DATA_DIRS` has the packaged one. Its `Exec` is the daemon's absolute path, because GLib rejects an entry whose program is not on the portal's `PATH`. The backend then calls `org.freedesktop.host.portal.Registry.Register("dettivo", {})` on the connection that opens the session, before any portal request on it. A portal without the registry falls back to the systemd unit, and `hotkeys.status.identity` says which happened. A refusal names the fix: restart `xdg-desktop-portal` once the entry exists, and avoid setgid wrappers such as `newgrp`, which hide `/proc/<pid>` from the portal.
- **Shortcut ids carry the chord.** Ids are `<action>-<chord>` (`dictation-hold-F9`, `dictation-toggle-SUPER_CTRL_X`). A changed chord binds a new id in a new session. The portal has no unbind call, so closing the old session is what drops the old id. `hotkeys.status.bound` keeps the action ids.
- **Empty triggers mean the compositor binds.** When every shortcut comes back without a trigger, the daemon reports `compositor_bound` and leaves `bound` empty, so the first-run Keys step still asks for the snippet. It never reports this as a refusal.

Two parts of the spec's approach are left out, and both are recorded as follow-ups:

- **A plain Escape that cancels only while a take records (Hyprland submap).** A submap entered by the press binding has to be left when the take ends any other way. Hyprland runs only the submap's own binds while it is active, so a submap left behind disables every other keybind until Escape. Leaving it reliably needs the daemon to reset the compositor's submap when a session ends. That is a second compositor writer, and it needs its own design.
- **A keyless "Cancel dictation" portal shortcut.** GNOME and KDE already let the user reassign the cancel shortcut the daemon binds. A second shortcut without a key adds nothing until a desktop can scope a key to "while recording".

## Consequences

- A user who taps the hold key by accident loses that take. The first 300 ms of a hold are too short to carry speech.
- A slow second press of a double tap (more than 300 ms) starts an ordinary hold.
- With `[hotkeys] backend = "auto"` on Hyprland, the portal session now opens. It binds the four ids without keys and delivers nothing unless the user binds them in Hyprland as `global` shortcuts. The snippet stays the path.
- `dettivo dictation release` waits for the take to settle before it answers, as `stop` did. The portal and evdev paths settle on a thread of their own.
- Tests: the machine's transitions are table tests in `crates/dettivo-hotkeys/src/hold.rs`. The mock portal refuses a session to an unregistered connection, and `crates/dettivo-hotkeys/tests/portal.rs` covers the rebind, the older portal and the empty triggers. `crates/dettivod/tests/hold.rs` drives the verbs through a lost release and a lock, and `crates/dettivod/tests/hotkeys.rs` drives them through the portal.
