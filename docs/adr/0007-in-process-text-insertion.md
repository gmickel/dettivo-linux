# 0007. Text insertion through an in-process Wayland virtual keyboard, with fallbacks

Status: Accepted 2026-09-03; the hand-over on any mid-way failure is superseded by [0047](0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md)

## What this gives you

Dictated text types into terminals, editors, browsers and Electron apps on Hyprland with nothing else installed, Unicode and non-US layouts included, and when a window refuses typed input you still get the text on the clipboard with a one-line explanation of what happened.

## Situation

No universal insertion API exists on Wayland. wlroots compositors (Hyprland, Sway, Niri, River) and KDE implement `zwp_virtual_keyboard_v1`; GNOME does not, and offers input only through the RemoteDesktop portal and libei. `wtype` implements the virtual keyboard protocol as an external binary and generates a keymap for arbitrary characters. `ydotool` needs a uinput daemon and group membership. The Windows port learned that insertion needs target verification, never a global paste, and never a success report after pasting into Dettivo's own window.

## Decision

`dettivo-insert` implements the virtual keyboard protocol in process (`wayland-client` plus `wayland-protocols-misc`) and uploads a generated xkb keymap covering every character in the text. The fallback chain is libei through the RemoteDesktop portal (`ashpd` plus `reis`) on GNOME and KDE, `ydotool` when its socket exists, `xdotool` on X11, clipboard plus an app-aware paste keystroke (`Ctrl+Shift+V` in terminals), then clipboard only. The target app id and pid are captured at hotkey press and re-verified before insertion; the chosen backend is reported in `InsertionResult`.

## Consequences

- No external binary is required on Omarchy; `wtype` and `ydotool` become optional backends.
- An X client under XWayland keeps the seat's keymap and never sees the generated one (measured on Hyprland with `wtype` as well), so the chain skips the virtual keyboard for such windows and types through `xdotool`; without it the text goes to the clipboard.
- Secure fields identified through AT-SPI are never typed into.
- Clipboard contents are restored after a clipboard-based insertion.
- The insertion matrix (terminals, Chromium, Electron, Qt, GTK, Firefox) is a release artifact that records backend, latency and outcome per target, because reliability here is the product's promise.
