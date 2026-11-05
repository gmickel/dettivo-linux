# Dettivo survives a compositor restart

## Goal & Context

When Hyprland restarts (a crash, a watchdog restart, `hyprctl reload` gone wrong), every Dettivo dictation afterwards goes to the clipboard until the user restarts the daemon, and often not even then. On 2026-11-02 Gordon's Hyprland was restarted by its watchdog; the running daemon kept the old `HYPRLAND_INSTANCE_SIGNATURE`, its focus probe got "Connection refused" on the dead instance's socket, it could not verify the focused window, so it fell back to `clipboard_only` and the pinned `virtual_keyboard` backend failed every take ("Not inserted: pinned backend virtual_keyboard is unavailable"). Restarting the service did not help, because the systemd user environment (exported by uwsm at login) still held the dead signature.

A dictation daemon that silently degrades until a manual fix is not best-in-class. This spec makes every compositor-facing part of Dettivo find the live compositor and reconnect on its own.

Origin: Gordon, 2026-11-02, goal fn-88 ("fold it in yes").

## Approach

- **Find the live Hyprland instance.** Treat `HYPRLAND_INSTANCE_SIGNATURE` as a hint, not the truth: if its socket is missing or refuses connections, look under `$XDG_RUNTIME_DIR/hypr/` for instances whose `.socket.sock` accepts a connection, and use the one that matches the current Wayland display (or the newest live one when only one is live). Re-resolve on every connection failure, not only at start.
- **Reconnect the Wayland pieces.** The virtual keyboard, the focus/lock-notify watchers and any other Wayland client connection reconnect when the compositor goes away and a new one appears on the display socket, with backoff; a dead connection never leaves a backend permanently "unavailable".
- **Report it.** `dettivo doctor` names the compositor instance in use and whether the environment's signature was stale; the log says once when it switched instances.
- Cover the hotkey portal session too if it does not already rebind after a compositor restart (fn-82's portal backend).

## Acceptance

- **R1:** With the daemon running, restarting the compositor (a stand-in Hyprland IPC socket and a stand-in Wayland compositor on private sockets in tests and QA) leads the next dictation to type through the pinned/selected keystroke backend again within 2 s of the new compositor being up, with no daemon restart. A test covers it.
- **R2:** A daemon started with a stale `HYPRLAND_INSTANCE_SIGNATURE` in its environment finds the live instance and types normally. A test covers it.
- **R3:** The focus probe, the virtual keyboard, the Hyprland lock-notify watcher (fn-81) and the portal hotkey session each recover; a dead compositor connection never leaves a backend unavailable after a new compositor is up.
- **R4:** `dettivo doctor` reports the instance in use and a stale environment signature; `docs/insertion.md` and `docs/hotkeys.md` describe the recovery, and a changelog entry is added. An ADR records the discovery rule.

## Boundaries

- Never connect to or act on a compositor other than the user's current session's (match the Wayland display; never guess across users or seats).
- No change to the insertion safety rules: a target that cannot be verified still goes to the clipboard.
- Tests and QA never touch the live compositor: stand-ins on private sockets only.

## Quick commands

- `cargo test -p dettivo-insert -p dettivo-hotkeys -p dettivod`
- `just build test lint`
