# Hold-to-talk that survives lost releases, with a working portal identity

## Goal & Context

Hold-to-talk has to work every time, on every compositor.

- **Portal identity.** On a stock Omarchy install, `dettivo doctor` reports `portal session refused: Portal request failed: org.freedesktop.portal.Error.NotAllowed: An app id is required`. A host (unsandboxed) app has no app id unless it registers one, so the GlobalShortcuts portal refuses the session.
- **Rebinding.** GNOME and KDE keep the first trigger they saw for a shortcut id and ignore a later `preferred_trigger`, so a rebind can silently do nothing.
- **Lost release.** Compositors can drop the key release (Hyprland Lua binds, GNOME), which leaves a hold session recording until `max_duration_seconds` (300 s).
- **Key repeat.** Some compositors repeat a held key as fresh presses.
- **Hands-free.** It needs a second chord today.

This spec fixes the portal identity and makes the hold key robust, with a double-tap lock for hands-free recording.

## Approach

**Register an app id before opening the portal session.**
- Ensure the desktop entry `dettivo.desktop` exists, so the app id is `dettivo`. The package ships it; for a source build the daemon writes a minimal one under `$XDG_DATA_HOME/applications/`.
- Call `org.freedesktop.host.portal.Registry.Register(app_id, {})` on the portal bus before `CreateSession`.
- An older portal without the registry falls back to the systemd unit name; report that in `doctor`.
- If registration fails, `doctor` gives the fix: restart `xdg-desktop-portal` after the desktop entry is written, and avoid setgid wrappers such as `newgrp`, which hide `/proc/<pid>` from the portal.

```rust
async fn register_host_app(conn: &zbus::Connection, app_id: &str) -> Result<(), PortalError> {
    let proxy = zbus::Proxy::new(conn, "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop", "org.freedesktop.host.portal.Registry").await?;
    match proxy.call_method("Register", &(app_id, HashMap::<&str, zvariant::Value>::new())).await {
        Ok(_) => Ok(()),
        Err(e) if is_unknown_method(&e) => Ok(()),        // old portal: identity from the unit
        Err(e) => Err(PortalError::Register(e)),
    }
}
```

**Put the trigger in the shortcut id.** Ids become `<action>-<normalised chord>` (`dictation-hold-F9`, `dictation-toggle-SUPER_CTRL_X`), so a changed chord registers a new shortcut that the desktop honours. The old id is unbound on rebind.

**Hyprland's portal.** It returns empty triggers because the user binds in the config. Treat empty triggers as "bound by the compositor" and keep the generated snippet as the binding path. Never report this as a refusal.

**One state machine for the hold key** (daemon side, fed by press and release from any backend: compositor binding CLI calls, portal `Activated`/`Deactivated`, evdev).

```rust
enum HoldState { Idle, Held { since: MonoInstant, second_tap: bool }, Locked, LastTap { at: MonoInstant } }

const DOUBLE_TAP: Duration = Duration::from_millis(300);
const REPEAT_FILTER: Duration = Duration::from_secs(1);

fn on_press(s: HoldState, now: MonoInstant) -> (HoldState, Action) {
    match s {
        HoldState::Idle => (Held { since: now, second_tap: false }, Action::Start),
        HoldState::LastTap { at } if now - at <= DOUBLE_TAP => (Held { since: now, second_tap: true }, Action::Start),
        HoldState::Held { since, .. } if now - since < REPEAT_FILTER => (s, Action::None),   // key repeat
        HoldState::Held { .. } => (Idle, Action::Stop),        // the release was lost: this press finishes
        HoldState::Locked => (Idle, Action::Stop),             // hands-free: next press finishes
        _ => (Held { since: now, second_tap: false }, Action::Start),
    }
}
fn on_release(s: HoldState, now: MonoInstant) -> (HoldState, Action) {
    match s {
        HoldState::Held { since, second_tap: true } => (Locked, Action::None),            // double-tap locks
        HoldState::Held { since, .. } if now - since < MIN_HOLD => (LastTap { at: now }, Action::DiscardTap),
        HoldState::Held { .. } => (Idle, Action::Stop),
        other => (other, Action::None),
    }
}
```

The compositor bindings call new idempotent verbs, `dettivo dictation press` and `dettivo dictation release`, which feed this machine. `start` and `stop` keep their current contract for scripts. `dettivo setup hyprland|sway` writes the new verbs. Niri has no release bindings, so it keeps `toggle`.

**Escape cancels while recording.** Register a keyless "Cancel dictation" portal shortcut for desktops whose portal binds it. Compositor snippets bind the cancel chord only while a session runs, where the compositor supports submaps (Hyprland). Otherwise they keep the existing cancel chord.

## Acceptance

- **R1:** On Omarchy with a stock xdg-desktop-portal, `dettivo doctor` reports the portal session as bound, not refused. The app id is registered before the session opens, and the desktop entry exists.
- **R2:** Changing `[hotkeys] hold` on GNOME or KDE takes effect after the daemon reloads its config, without a manual unbind. The old shortcut id is removed.
- **R3:** Empty triggers from Hyprland's portal are reported as "bound by the compositor", and dictation still works through the generated snippet.
- **R4:** A lost release, meaning a second press while a hold session runs and more than 1 s after its start, finishes the take. A repeated press within 1 s is ignored. Both have unit tests on the state machine and a QA drive with a synthetic missing release.
- **R5:** Tap, then press again within 300 ms, locks recording. Releasing does not stop a locked session, and the next press finishes it. The pill shows the locked state.
- **R6:** `dettivo dictation press` and `release` are idempotent and documented. `dettivo setup` writes them for compositors with release bindings, and existing `start`/`stop` bindings keep working.
- **R7:** `docs/hotkeys.md` documents the state machine, the timings and the portal identity. A changelog entry is added.

## Boundaries

- Onboarding's F9 activation path stays as it is (separate spec).
- No new raw-input requirement: evdev stays opt-in, and the default path needs no `input` group.

## Quick commands

- `cargo test -p dettivo-hotkeys -p dettivod`
- `just build test lint`
