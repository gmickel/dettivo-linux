# 0091. Delivery restores the whole clipboard, joins continuations and ducks media

Status: Accepted 2026-11-01

## What this gives you

A dictation leaves the desktop as it found it. An image or rich text on the clipboard comes back after a paste. Two dictations into one field read as one sentence. Playing media can be lowered or muted while you speak, and a tap never touches it. A script can read a dictation from `dettivo dictation stop --print`.

## Situation

fn-85 listed four gaps. The `clipboard_paste` restore read and wrote text only, so an image or a file list was lost. Two dictations in a row each started with a capital and no joining space. A shell script needed a socket client to get the processed text. `[hotkeys] pause_media` could pause media but not lower it.

The libraries set some of the shape. `wl-clipboard-rs` 0.9.3 reads and offers any MIME type through `ext_data_control_v1` or `zwlr_data_control_v1`, and its serving call returns when another client takes the clipboard. `x11-clipboard` 0.9.3 serves one target per selection and links `x11rb` 0.13 while the workspace uses 0.14. Hyprland reports input activity through `ext_idle_notifier_v1`. Version 2 of that protocol adds an input-idle notification that idle inhibitors do not hold back.

## Decision

**Clipboard.** `crates/dettivo-insert/src/clip` defines `Clip { snapshot, set_text, restore }`. A snapshot reads every offered type within 1 s and 32 MiB, leaving out a type that does not fit or answer. `set_text` returns a generation. `restore` offers the snapshot again only while that generation is still the clipboard. On Wayland that means the thread serving Dettivo's offer has not returned. Under X11 it means no `SelectionClear` arrived at Dettivo's own owner window. X11 runs on `x11rb` 0.14 directly: reads follow `INCR`, and one owner window per process serves every restored target. `x11-clipboard` is removed. A clipboard that could not be read is not restored. The restore stays synchronous inside the insertion, so a second dictation starts after the first restore and keeps the first original. `clipboard_restore_delay_ms` defaults to 500 ms.

**Joins.** `crates/dettivo-insert/src/join.rs` keeps Dettivo's last delivery: its text, the window it was typed into (app id, pid and window identity), and when. A take joins when it lands in that window within 30 s and no input arrived between that delivery and 1 s before the take started. The 1 s leaves out the hotkey. A paste counts as delivered when the paste keystroke went out, before the clipboard restore delay, so typing during that delay is seen. The joined text starts with a space unless the previous text ended with a newline or space. It keeps its capital only after `.`, `!` or `?`, and otherwise loses the capital of a leading "The", "And", "But", "So", "Or", "A" or "An". Input comes from `crates/dettivo-insert/src/idle.rs`, which records each `resumed` of a version 2 input-idle notification with a 100 ms timeout. Without version 2 there is no signal, and nothing joins. The decision never reads the target's text. `[dictation] join_continuations = false` turns it off. The joined text is what gets typed. The history keeps the take's own text.

**Print.** `dictation.stop` takes `print`. A print and the worker's commitment to insert race for one decision per take: a print before the commitment wins, and one after it is refused with `CONFLICT` kind `sessionDelivering`. The session finishes the take and runs the mode step, records the insertion as outcome and method `printed`, and returns the text in `StopResult.text`. A take the gate found silent answers `stop` with `CONFLICT` kind `noSpeech`, with or without `print`, and the CLI maps that to the new exit code 6. The CLI waits up to 60 s for a printing stop.

**Media.** `[audio] while_dictating` is `nothing`, `pause`, `duck` or `mute`, and `[hotkeys] pause_media = true` with `nothing` reads as `pause`. `crates/dettivod/src/media.rs` runs on its own thread. It acts only once a take has recorded 300 ms, the hold key's minimum hold (ADR 0083), so a discarded tap never touches media. Duck moves the default sink to `min(volume, 0.20)` along a smoothstep in ten steps over 200 ms through `wpctl`. The ramp stops when a step finds a volume other than the one it set. At the end, Dettivo ramps back up only when the volume is still the ducked one, unmutes only when the sink is still muted, and resumes only the players it paused. The volume and mute go to the sink that was the default when the take started, by its PipeWire id, in hundredths. The end of a take waits, up to 2 s, until media is back before the session frees its slot, and a meeting starts only on a free slot, so media never changes under a meeting's recording of the default sink's monitor.

## Consequences

- An X11 type larger than the server's request limit, 16 MiB with BIG-REQUESTS, is offered after a restore but refused when pasted, because the owner does not send `INCR`.
- Under X11 the owner window lives as long as the daemon. The restored content is gone when the daemon exits, as it is for any X11 selection owner.
- Any key or pointer input counts as input for a join, so moving the mouse between two dictations prevents the join. Input in the last second before a take starts is not seen. Typing that continues straight from Dettivo's own delivery, with no pause of 100 ms, is not seen either.
- A compositor without `ext_idle_notifier_v1` version 2 never joins dictations.
- `printed` is a value outside the macOS contract's `InsertionOutcome` and `InsertionMethod`. `docs/api/linux-deltas.md` lists it.
- Ducking reads and sets the volume through `wpctl` subprocesses, two per ramp step. That is about twenty process starts per ramp, all off the capture path.
- The R1 and R2 QA drives on a live Hyprland desktop were not run for this change. Every live-desktop check waits for Gordon after the 2026-11-01 compositor crash. The X11 clipboard round trip (a 1.5 MB image from `xclip`, a two-type offer, a copy during the delay) ran against a private Xvfb display.
