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
| `clipboard_paste` | Takes a snapshot of the clipboard in every type it offers, sets the text, sends the paste keystroke through the first keystroke backend that reaches the window (`ctrl+shift+v` in terminals, `ctrl+v` elsewhere, `[insert] paste_keys` per app id), then puts the snapshot back after `clipboard_restore_delay_ms` (see [The clipboard comes back whole](#the-clipboard-comes-back-whole)). | A Wayland or X11 clipboard. |
| `clipboard` | Sets the clipboard and stops. The only backend allowed when the target cannot be verified, and the one `clipboard_only` mode uses. | A Wayland or X11 clipboard. |

Right before a backend delivers, and between its typing batches, the daemon probes the focus again: the window must still be the one decided on above, so a portal dialog or a long take cannot hand the text to another window, Dettivo's own included. A focus that moved before the first key is `failed` with reason `target_changed: ...` and nothing is typed; one that moved between batches ends the typing there and reports it. A backend that stopped before delivering anything hands over to the next one; a backend that may have typed a prefix (a helper that exited with an error or ran out of time, a compositor that dropped the connection mid-text) ends the insertion as `failed` with reason `partial_delivery: <backend>: ...`, and the text is never typed a second time. Insertions run one at a time. Every skipped backend is logged with its reason and the text is never logged. `[insert] backend` pins one backend, and a pinned backend that is unavailable fails with its reason instead of falling back.

5. The result is the contract's `InsertionResult` plus a Linux `backend` block: the backend name, its latency in milliseconds and whether `insert.undo` can take the text back.

## The clipboard comes back whole

An image, a file list or rich text you copied before a `clipboard_paste` dictation is on the clipboard again once the paste is done, in every type it was offered as ([ADR 0091](adr/0091-delivery-restores-the-whole-clipboard-joins-continuations-and-ducks-media.md)).

- Before the text is set, Dettivo reads every type the clipboard offers, up to 32 MiB in total and within 1 s. On Wayland it reads through data-control (`ext_data_control_v1`, else `zwlr_data_control_v1`), and under X11 through the `CLIPBOARD` selection, following `INCR` transfers. A type that does not fit or does not answer in time is left out of the snapshot and logged by name.
- `clipboard_restore_delay_ms` (500 ms) after the paste keystroke, Dettivo offers the snapshot again. An empty clipboard is cleared again.
- A copy made during the delay, by you or by another app, wins: Dettivo restores only while the text it set is still the clipboard. On Wayland it knows that because its own offer is still being served, and under X11 because the X server still names its window as the `CLIPBOARD` owner, checked under a server grab so no copy can slip in between.
- A clipboard that could not be read at all is not restored, so the dictated text stays rather than an empty clipboard.
- Two dictations in a row keep your first original, because the second one starts only after the first restore.

Under X11, Dettivo serves the restored types from a window of its own for as long as the daemon runs, each in the property type and format the original owner answered with. The dictated text is offered as `UTF8_STRING`, `TEXT` and the `text/plain` types, and as `STRING` only when it has an ISO-8859-1 form. A single type larger than the X server's request limit (16 MiB with BIG-REQUESTS) is offered but answered with a refusal, because Dettivo does not send `INCR`. `[insert] restore_clipboard = false` leaves the dictated text on the clipboard.

## Held modifiers wait out

The first key of a dictation goes out at least 250 ms after the stop. When a toggle chord such as Super+Ctrl+X stops a take, Super and Ctrl may still be down, and a virtual keyboard's keys pressed with them would arrive as shortcuts. The wait runs alongside the transcription, which almost always takes longer, so it rarely adds time. No source on Hyprland reports which keys are held ([ADR 0090](adr/0090-the-first-key-waits-a-fixed-250-ms-after-the-stop.md)), so the wait is fixed, and a modifier still held 250 ms after the stop still reaches the target. `dettivo dictation stop --print` sends no keys and skips the wait.

## Consecutive dictations join

Two dictations into the same window read as one sentence when the second continues the first ([ADR 0091](adr/0091-delivery-restores-the-whole-clipboard-joins-continuations-and-ducks-media.md)). When a take lands in the window the previous one was typed into, within 30 s of it, and you gave no input in between, the take is joined to it:

- it starts with one space, unless the previous text ended with a newline or a space;
- it keeps its capital only when the previous text ended a sentence (`.`, `!` or `?`);
- otherwise a leading "The", "And", "But", "So", "Or", "A" or "An" is lowercased. Other words keep their capital, since they may be names.

"I wrote the report" followed by "And then I left." types " and then I left." The join reads only Dettivo's own last delivery and never the text in the window. The window is the one the probe named when the key went down, identity included, so a second window of the same app does not join.

Dettivo learns whether you gave input in between from the compositor's input-idle notification (`ext_idle_notifier_v1` version 2): any key or pointer input that follows 100 ms without input, between the last delivery and 1 s before the next take started, counts as input. The 1 s leaves out the key that started the take. A compositor without that protocol version gives no signal, and then nothing is joined. A take that went to the clipboard, or a `reinsert-last`, is never joined. `[dictation] join_continuations = false` types every dictation as it is.

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
