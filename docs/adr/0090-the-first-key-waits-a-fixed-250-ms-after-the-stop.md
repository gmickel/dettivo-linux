# 0090. The first key of a dictation waits a fixed 250 ms after the stop

Status: Accepted 2026-11-01

## What this gives you

A dictation stopped with a chord such as Super+Ctrl+X types its text as text. Super and Ctrl, still held from the chord, no longer turn the first characters into shortcuts.

## Situation

On Hyprland a virtual keyboard's keys merge with the modifiers the user still holds. A toggle chord stops the take on the key press, so Super and Ctrl are often still down when a short take is transcribed and typed. fn-85 R2 asked the delivery to wait until no modifier is held, or 250 ms after the hotkey's release, capped at 2 s, and to send the text to the clipboard with reason `modifiers_held` when the modifiers stay down past the cap.

No source on this desktop reports held keys. `hyprctl devices -j` carries no modifier state on Hyprland 0.56.2. The Lua call `hl.is_key_down` crashed the live compositor when an agent probed it on 2026-11-01. The user is not in the `input` group, so evdev's `EVIOCGKEY` is unreadable. A compositor binding sends the daemon no release event, so "250 ms after the release" has no input either. The goal log for fn-88 records the three options put to Gordon: (a) a fixed settle delay after the stop with the 2 s cap and no key-state source, (b) evdev key state with the user added to the `input` group, (c) a narrower R2. Gordon chose (a) on 2026-11-01.

## Decision

The session worker delays its hand-over to the inserter until `MODIFIER_SETTLE` (250 ms) has passed since the stop it observed (`crates/dettivo-session/src/deliver.rs`). The delay applies to every stop: a hold chord with a modifier leaves the same modifier down on release, and a fixed delay costs a stop without a chord nothing it can notice. The wait never exceeds 250 ms, well inside R2's 2 s cap. Dettivo never asks the compositor, the portal, X11 or evdev for key state.

`dettivo dictation stop --print` sends no keys, so it skips the wait.

## Consequences

- The wait runs alongside the transcription and adds time only when the engine finishes within 250 ms of the stop, which a warm engine on a short take can do. `crates/dettivo-session/tests/deliver.rs` pins both cases: an engine that answers at once still waits until 250 ms after the stop, and a 400 ms engine adds nothing.
- R2's clipboard fallback with reason `modifiers_held` is not built, because nothing reports that the modifiers are still down. A user who keeps the chord held for longer than 250 ms after the stop can still get shortcuts. A key-state source, if one appears (an `input` group member, a compositor that reports modifiers safely), would add the wait-until-released and the fallback on top of this delay.
- The 250 ms value is Gordon's estimate. A measurement on Hyprland of how long the chord stays down after the stop would tune it.
