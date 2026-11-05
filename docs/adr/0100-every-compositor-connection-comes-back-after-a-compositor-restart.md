# 0100. Every compositor connection comes back after a compositor restart

Status: Accepted 2026-11-02

## What this gives you

When the compositor restarts on the same Wayland display, Dettivo keeps working without a daemon restart. The screen-lock watcher closes the always-ready microphone on the next lock, consecutive dictations join again, and portal hotkeys reach the daemon again.

## Situation

Three of the daemon's connections outlived a single compositor.

- The screen-lock watcher (`crates/dettivod/src/ready_screen.rs`, ADR 0092) retried every 5 s, but it reconnected with `wayland_client::Connection::connect_to_env`, which reports a refused display socket as `NoCompositor`, the same error as an unset `WAYLAND_DISPLAY`. The watcher took that as "no display" and stopped for good when its retry landed while the compositor was down.
- The input-idle watch (`crates/dettivo-insert/src/idle.rs`, ADR 0091) connected once and stopped at the first lost connection, so joins ended until the daemon restarted.
- The GlobalShortcuts session (ADR 0083) lives in the portal backend. On Hyprland that backend is `xdg-desktop-portal-hyprland`, which exits with the compositor and comes back without the daemon's session or bindings.

The virtual keyboard and the clipboard connect once per insertion, so they reach a restarted compositor on their own. The focus probe is ADR 0099.

## Decision

- `crates/dettivo-insert/src/display.rs` connects to the display socket by path (`WAYLAND_DISPLAY` under `XDG_RUNTIME_DIR`) and runs the long-lived watchers in one loop. A refused or lost connection is tried again after 100 ms, the wait doubling to a 2 s cap and starting over after a connection that served 10 s. A compositor that does not offer the watched protocol ends the loop, as before. Only an unset `WAYLAND_DISPLAY` means there is no display.
- The screen-lock watcher and the input-idle watch both run in that loop. A lost lock watcher keeps the ring closed until the new compositor reports, whether it last saw a lock or an unlock, so a compositor that comes back locked never finds the microphone open. The input-idle watch answers only for spans that start after its current connection began, so input made while the compositor was away never reads as a quiet continuation and the next take is not joined.
- The portal backend follows `NameOwnerChanged` for the bus names `org.freedesktop.impl.portal.desktop.*`. When a name it saw lose its owner gets one again, the daemon closes its old session, opens a new one and binds the same shortcuts, trying 5 times with the wait doubling from 250 ms. Only a failed request is tried again: a bind the portal answers with a refusal (the person dismissed or denied the dialog, or left it unanswered for 120 s) is final, so no second dialog follows it. Every wait in the rebind gives way to a stop within 50 ms and closes the session it opened, so a `[hotkeys]` reload never waits on a dialog. The app id stays registered with the frontend, which did not restart, so the daemon does not register it again.

## Consequences

- A compositor that is back is followed again within 2 s at most. The lock watcher's ring stays closed meanwhile, so a restart never opens the microphone during a lock.
- A portal backend that starts for the first time (a file chooser starting `xdg-desktop-portal-gtk`, say) triggers nothing, because only a backend seen going away gets a rebind. A rebind that fails 5 times, or is refused, leaves the shortcuts unbound until the backend restarts again or `[hotkeys]` changes. `hotkeys.status` and `dettivo doctor` then show nothing bound and name the failure in `error`; a rebind that succeeds shows the new bindings.
- A restart of `xdg-desktop-portal` itself, the frontend, is not followed.
- `crates/dettivo-insert/src/stand_in/` holds a stand-in Wayland compositor and Hyprland instance on private sockets, built only for tests (feature `stand-in`). The lock watcher, the input-idle watch, the portal backend (against the mock portal) and the daemon are each tested across a restart.
