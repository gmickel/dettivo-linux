# 0016. Compositor bindings drive the CLI; the portal and evdev backends live in the daemon; every start captures the focused window

Status: Accepted 2026-09-04; the target as an app id and a pid is superseded by [0047](0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md)

## What this gives you

On Omarchy you hold `F9` and talk, exactly as with Voxtype, after one command and one line in `hyprland.lua`; on GNOME and KDE the same four actions come through the portal without compositor-specific code; and the text of a take only ever lands in the window that was focused when the key went down.

## Situation

Hyprland has no GlobalShortcuts portal for a caller without an app id (its portal answers `NotAllowed`, and a systemd user service has no app id), while GNOME and KDE implement the portal and offer no other global binding path. Hyprland 0.56 and Omarchy moved to a Lua configuration (`hl.bind` with `release = true`) while classic `hyprland.conf` installs still exist; Sway has `bindsym --release`; Niri has no release bindings at all. Omarchy binds `F9` (hold) and `Super+Ctrl+X` (toggle) to Voxtype when it is installed, and `Super+Ctrl+R` to its reminder menu. The insertion guards from ADR 0007 need the target from hotkey time, which the dictation session did not carry. Injected keys through the Wayland virtual keyboard protocol do not trigger Hyprland's bindings; keys through `/dev/uinput` do.

## Decision

Compositor bindings are the primary path and are not a daemon backend: `dettivo setup hyprland|sway|niri` writes a snippet under `$XDG_CONFIG_HOME` that runs `dettivo --quiet dictation start|stop|toggle|cancel|reinsert-last`, activates the Lua include and reloads Hyprland on Omarchy, and prints the manual include for other compositors. The Lua setup preserves unrelated main configuration and appends its include only when missing (amended 2026-09-14, [ADR 0030](0030-omarchy-plugin-in-repo-folder-mirror-and-panel-hosted-pill.md)). `hyprland` renders the Lua flavour when `hyprland.lua` exists and the classic one otherwise; the Hyprland snippets `unbind` the two Voxtype chords first. The defaults are Omarchy's: `hold = "F9"`, `toggle = "SUPER CTRL, X"`, `cancel = "SUPER CTRL, Escape"`, `reinsert = "SUPER CTRL SHIFT, X"`; chords use the classic Hyprland notation and accept the Lua spelling. `dettivo-hotkeys` holds the `Action` enum, the `HotkeyBackend` trait, the chord parser and renderers, and two backends: the portal's GlobalShortcuts through `ashpd` and evdev through the `evdev` crate; `[hotkeys] backend = "auto"` takes the portal when the desktop answers and otherwise nothing. `F13` to `F24` are not chord names, because the standard xkb rules deliver those evdev codes as `XF86Tools` and friends.

Every start path (the CLI, the portal, evdev) probes the focused window before the microphone opens and stores it on the session; the session's inserter is the ADR 0007 chain with that window as the guard, so a moved focus refuses the insertion with `CONFLICT` and keeps the transcript. `dictation.start` accepts `expected_target_bundle_id` and `expected_target_pid` for a caller that captured the window itself, `dictation.status` reports `target`, and `dictation.stop` reports the `insertion` block. A hotkey press while a session runs is ignored with a log line. MPRIS players that are playing are paused on capture start and resumed at the end when `pause_media` is on; start, stop and error cues play through `pw-play` when `sounds` is on. `hotkeys.status` and `system.capabilities.hotkeys` report the backend, the availability of both paths with reasons, and the bound actions; `dettivo doctor` adds whether the snippet is loaded.

The QA drive presses real keys through a `uinput` virtual keyboard, and the daemon's portal and MPRIS tests run against mock implementations on a private `dbus-daemon` whose configuration names no service directory, so the desktop's real portal is never activated by a test.

## Consequences

- Omarchy users run `dettivo setup hyprland`, add one `dofile` line after Omarchy's defaults, and `hyprctl reload`; Voxtype's keys are taken over by the snippet's `unbind` lines.
- Hyprland and Sway fire the release binding when the chord's key comes up, so the modifiers are released last; Niri users toggle instead of holding.
- On Hyprland the portal backend stays unavailable and `auto` reports the reason once; the portal serves GNOME and KDE. A daemon that runs outside an application scope has no app id for the portal, so the portal path presumes a launcher that provides one.
- The evdev backend needs the `input` group; it is off by default and every refusal names the requirement.
- `dictation.stop` no longer fails a session whose insertion was refused: the transcript is kept and `insertion.reason` starts with `CONFLICT`.
- Feedback sounds need `pw-play`; the in-process PipeWire playback stream is not built. Cues are silent during a meeting once the meeting flag exists (S-23).
- Test daemons never see `DBUS_SESSION_BUS_ADDRESS`; the QA profile pins `[hotkeys] backend = "none"`.
