# Text insertion

Dictated text lands in the window you were typing in, on Hyprland with nothing else installed, Unicode and non-US layouts included. When a window refuses typed input the text is still on your clipboard and the result says which backend ran and why the others did not (ADR 0007).

## What happens on an insertion

1. The daemon looks at the focused window through the probe for your session: Hyprland IPC on Hyprland, the X11 active window under XWayland or a plain X session. The window's app id and pid come back as the `target`; the title is hashed and never leaves the daemon, and the window's own identity (the Hyprland address, the X11 window id) stays inside the daemon as the guard that tells two windows of one process apart.
2. The guards a client captured at hotkey time (`expected_target_bundle_id`, `expected_target_pid`), or the window a dictation session recorded when its key went down ([docs/hotkeys.md](hotkeys.md), identity included), are compared with that target. A mismatch is `CONFLICT` and nothing is typed or copied. A guard on a session with no probe is a conflict too: an unverifiable target never receives a paste. A dictation whose origin the probe could not name when the key went down (the probe failed, or nothing was focused) is `copied_to_clipboard` with reason `origin_unverified`, never typed into whatever window has the focus by the time the take is transcribed.
3. One of Dettivo's own windows as the target answers `failed` with reason `target_is_self`, before any backend runs. The one exception is the first-run Try it step, which dictates into its own field: the app arms `insert.allow_self_target` on its connection while the step is on screen, and only then does a take whose key went down over the app's window (never the pill's) land in it; the allowance ends when the step leaves or the app's connection closes, and a peer that is not `dettivo-app` cannot arm it (ADR 0024).
4. The chain picks the first backend that is available, can type this text, and reaches this window:

| Backend | How | Needs |
|---|---|---|
| `virtual_keyboard` | `zwp_virtual_keyboard_v1` in process: a keymap generated for exactly the characters of the text is uploaded and its keys are pressed, in batches of 160 distinct characters. Undo unavailable. | A Wayland compositor with the protocol (Hyprland, Sway, Niri, River, KDE). Not for XWayland windows: an X client keeps the seat's keymap, so the chain skips it there. |
| `libei` | The RemoteDesktop portal hands over an EI socket; the text goes through `ei_text` (libei 1.6) or through keycodes of the server's keymap. | A portal with `RemoteDesktop` version 2 (GNOME, KDE). The first use asks for permission; a refusal is remembered for the session. |
| `ydotool` | `ydotool type`, the text on standard input. | `ydotool` on `PATH` and `ydotoold` running (its socket exists). Never installed or configured by Dettivo. |
| `xdotool` | `xdotool type` on the X display (XTEST). The path for XWayland windows and for the CI drive under Xvfb. | `DISPLAY` set and `xdotool` on `PATH`. |
| `clipboard_paste` | Sets the clipboard, sends the paste keystroke through the first keystroke backend that reaches the window (`ctrl+shift+v` in terminals, `ctrl+v` elsewhere, `[insert] paste_keys` per app id), then restores the previous clipboard after `clipboard_restore_delay_ms` unless you changed it meanwhile. | A Wayland or X11 clipboard. |
| `clipboard` | Sets the clipboard and stops. The only backend allowed when the target cannot be verified, and the one `clipboard_only` mode uses. | A Wayland or X11 clipboard. |

Right before a backend delivers, and between its typing batches, the daemon probes the focus again: the window must still be the one decided on above, so a portal dialog or a long take cannot hand the text to another window, Dettivo's own included. A focus that moved before the first key is `failed` with reason `target_changed: ...` and nothing is typed; one that moved between batches ends the typing there and reports it. A backend that stopped before delivering anything hands over to the next one; a backend that may have typed a prefix (a helper that exited with an error or ran out of time, a compositor that dropped the connection mid-text) ends the insertion as `failed` with reason `partial_delivery: <backend>: ...`, and the text is never typed a second time. Insertions run one at a time. Every skipped backend is logged with its reason and the text is never logged. `[insert] backend` pins one backend, and a pinned backend that is unavailable fails with its reason instead of falling back.

5. The result is the contract's `InsertionResult` plus a Linux `backend` block: the backend name, its latency in milliseconds and whether `insert.undo` can take the text back.

## Undo

`dettivo insert undo` remains available as a protocol operation, but the built-in backends report `undo_supported = false` and refuse with `unsupported_backend`. Synthesizing Shift+Left and Delete cannot verify the inserted range after cursor movement, intervening typing or a same-process window switch. The OSD offers "copy again" instead. A future backend must identify the range or use an application transaction before advertising undo ([ADR 0054](adr/0054-delete-duplicate-work-and-keep-one-owner.md)).

## Seeing what the chain would do

```
dettivo insert target          # the focused window, every backend with its reason, the chain's choice
dettivo doctor                 # the same backend lines inside the full report
dettivo status capabilities    # platform.insertion_backend names the choice
```

`dettivo insert --text "Hello"` inserts through the chain; `--expected-target-bundle-id` and `--expected-target-pid` carry the guards a compositor binding captured; `--mode clipboard_only` copies without a paste keystroke; `--text -` reads standard input. Every key of `[insert]` is in [docs/config.md](config.md).

## Proving it

The QA rig drives the chain against real windows: `dettivo-qa drive insertion_matrix` inserts a sample with an accent and CJK into the Qt insert target and into every installed terminal (`foot`, `ghostty`, `alacritty`, `kitty`), reads the text back, and writes `insertion-matrix.json` with backend, latency and outcome per target; a target that is not installed or not reachable in the session is recorded as `skipped` with the reason. `dettivo-qa drive never_into_self` proves that a Dettivo window as target is refused and stays empty. The CI drive job runs both under Xvfb, where the Qt target goes through `xdotool`; on a Hyprland desktop the terminals go through the virtual keyboard and the Qt target, an XWayland window, through `xdotool`.
