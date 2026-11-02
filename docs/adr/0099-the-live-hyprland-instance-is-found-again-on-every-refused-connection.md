# 0099. The live Hyprland instance is found again on every refused connection

Status: Accepted 2026-11-02

## What this gives you

After Hyprland restarts, the next dictation types into the focused window again, with no daemon restart and no change to the systemd user environment. `dettivo doctor` says which Hyprland instance the daemon talks to and whether the signature in its environment is stale.

## Situation

On 2026-11-02 Gordon's Hyprland was restarted by its watchdog. The new instance got a new signature. The running daemon kept the old `HYPRLAND_INSTANCE_SIGNATURE`, its focus probe got "Connection refused" on the dead instance's `.socket.sock`, and with no verified target the chain allowed only the clipboard. The pinned `virtual_keyboard` backend then failed every take with "pinned backend virtual_keyboard is unavailable: target cannot be verified; clipboard only". Restarting the service did not help, because uwsm exported the signature into the systemd user environment at login and the new daemon inherited the dead one from there.

Hyprland keeps one directory per instance under `$XDG_RUNTIME_DIR/hypr/<signature>/`. Its `hyprland.lock` holds the compositor's pid on the first line and its Wayland display on the second; `.socket.sock` is the IPC socket. A crashed instance leaves both behind, and its socket refuses connections. The alternatives were to rewrite the environment (Dettivo never changes the systemd user environment), to poll for new instances on a timer, or to pick the newest directory without checking it.

## Decision

`crates/dettivo-insert/src/instance.rs` owns the instance in use for the daemon's insertion service.

- The environment's signature is a hint. The probe connects to the instance in use, which at first is the environment's, unless that instance's `hyprland.lock` names a different Wayland display; then the hint is ignored and the search below runs at once. A lock with no display line, as older Hyprland releases write, cannot be checked, so the environment's instance is used as before.
- When that connection is refused or the socket is missing, the daemon reads every `hyprland.lock` under `$XDG_RUNTIME_DIR/hypr/`, keeps the instances whose second line equals the daemon's `WAYLAND_DISPLAY`, and connects to them newest lock first. The first that accepts becomes the instance in use, and its connection carries the query. The search runs on every refused connection, never on a timer.
- An instance of another Wayland display is never connected to, and nothing outside this user's runtime directory is read. With no instance of this display answering, the probe fails and the existing rule holds: an unverifiable target gets the clipboard only.
- The log says `hyprland: switched to the live instance` once per switch, with both signatures, and warns once when no instance answers.
- `insert.target` carries a `compositor` block on Hyprland: `signature` (the instance in use, `null` when none answers), `environment_signature`, and `environment_stale`. `dettivo doctor` renders it as its `compositor` row.

## Consequences

- The first probe after a restart costs one refused connection and one directory read before it reaches the new instance. `crates/dettivod/tests/compositor_restart.rs` restarts a stand-in Hyprland and Wayland compositor under a running daemon and types the next insertion through the pinned virtual keyboard within 2 s; a second test starts a daemon with a dead signature in its environment, and a third with the signature of a live instance on another display, which is never queried.
- A Hyprland older than the `hyprland.lock` file with the display line is never matched, so it keeps the old behaviour after a restart: the clipboard until the daemon starts with a current signature.
- The daemon keeps a stale signature in its own environment. Child processes it starts inherit the stale value; none of them talks to Hyprland today.
- A daemon started without `HYPRLAND_INSTANCE_SIGNATURE` at all still does not look for Hyprland. The rule repairs a stale hint and never guesses that a session is Hyprland.
