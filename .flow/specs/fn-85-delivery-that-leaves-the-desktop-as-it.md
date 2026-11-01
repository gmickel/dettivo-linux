# Delivery that leaves the desktop as it found it

## Goal & Context

Dictated text should land cleanly and leave everything else untouched. Today four things fall short:

- **Clipboard restore.** After a `clipboard_paste` insertion Dettivo restores only text. An image, a file list or rich text on the clipboard is lost.
- **Held modifiers.** On Hyprland, modifiers still held from the hotkey chord (Super+Ctrl) merge into virtual-keyboard input, so typed text can arrive as shortcuts.
- **Consecutive dictations.** Two dictations in a row read as two sentences: each starts with a capital and no joining space.
- **Scripts and media.** Shell scripts and terminal agents can't get the processed text without a socket client. Playing media can only be paused, not lowered.

## Approach

**Full clipboard snapshot and guarded restore.**
- Before setting the clipboard, snapshot every offered MIME type through data-control, up to 32 MiB in total, with a 1 s read deadline. Use `ext_data_control_v1`, else `zwlr_data_control_v1`, else X11 selections.
- Restore after `clipboard_restore_delay_ms` (default 500 ms) by offering the snapshot again.
- A generation counter cancels the restore if anything newer was copied, by the user or another app. Back-to-back dictations keep the first original.

```rust
pub struct ClipboardSnapshot { pub offers: Vec<(String /* mime */, Vec<u8>)> }

pub trait Clip {
    fn snapshot(&self, budget: usize, deadline: Duration) -> Result<Option<ClipboardSnapshot>, ClipError>;
    fn set_text(&self, text: &str) -> Result<Generation, ClipError>;
    fn restore(&self, snap: ClipboardSnapshot, if_generation: Generation) -> Result<Restored, ClipError>; // Restored | Superseded
}
```

**Modifier settle before typing or pasting.**
- Before the first key a backend sends, wait until no modifier is held, or 250 ms after the hotkey's release event, capped at 2 s.
- Read the held state from the source that reports it: Hyprland IPC, the portal `Deactivated` signal, X11 `QueryPointer` mask, or evdev.
- If modifiers are still held at the cap, deliver through `clipboard` only, with reason `modifiers_held`.
- Reproduce the bug first in a QA drive. Typing while Super+Ctrl are held must produce no shortcuts.

**Continuation-aware joins.**
- When a dictation lands in the same window as the previous one, within a short window (default 30 s), and nothing was typed in between, join the takes:
  - the new take starts with a space unless the previous one ended with a newline;
  - it keeps its capital only if the previous take ended a sentence;
  - otherwise a leading "The", "And", "But", "So", "Or", "A" or "An" is lowercased.
- Nothing typed in between is known from the window guard plus an input-idle check where available. Without that signal, no join.
- `[dictation] join_continuations = true` turns it off.

```rust
fn join_prefix(prev: &LastDelivery, now: &Target, text: &str) -> Cow<'_, str> {
    if !prev.same_target(now) || prev.age() > JOIN_WINDOW || prev.user_typed_since { return text.into(); }
    let needs_space = !prev.text.ends_with(['\n', ' ']);
    let body = if prev.text.trim_end().ends_with(['.', '!', '?']) { text.into() } else { lower_leading_function_word(text) };
    if needs_space { format!(" {body}").into() } else { body }
}
```

**`dettivo dictation stop --print`.** Finish the take, run the mode step, print the final text to stdout, and insert nothing. It waits up to 60 s for transcription. The exit code says whether there was speech: 0 for text, and a new code 6 (`NoSpeech`) in the CLI exit map for no speech. The history item records `insertion = printed`.

**Media ducking.** Alongside `[hotkeys] pause_media`, add `[audio] while_dictating = "nothing" | "pause" | "duck" | "mute"`.
- Duck lowers the default sink to 20% along a smoothstep ramp over about 200 ms. If the user changes the volume during the ramp, the ramp gives up.
- Ducking applies only once the hold passes the 300 ms minimum, so taps never touch media.
- On restore, only what Dettivo changed is put back.
- Use PipeWire or `wpctl` for volume and MPRIS for pause. Run all of it off the dictation path, and never delay capture.

## Acceptance

- **R1:** An image on the clipboard before a `clipboard_paste` dictation is on the clipboard again after the restore delay, in every MIME type that was offered. A copy made during the delay wins over the restore. Both are covered in a QA drive on Hyprland.
- **R2:** With Super+Ctrl still held from the toggle chord, typed text arrives as text and triggers no shortcut. If modifiers stay held past 2 s, the text goes to the clipboard with reason `modifiers_held`.
- **R3:** Two dictations into the same field with nothing typed between them join with one space and correct capitalisation. Typing between them, or a different window, disables the join. `join_continuations = false` restores today's behaviour.
- **R4:** `dettivo dictation stop --print` prints the processed text to stdout, inserts nothing, records `printed` in history, and exits 6 (`NoSpeech`, documented in the CLI exit map) when no speech was heard.
- **R5:** `while_dictating = "duck"` lowers and restores the default sink smoothly, never overrides a volume the user set mid-take, and never acts on a tap. `pause_media` maps to `"pause"`.
- **R6:** `docs/insertion.md`, `docs/dictation.md` and `docs/config.md` describe all five behaviours. A changelog entry is added.

## Boundaries

- The insertion chain's order and its focus guards are unchanged.
- Join decisions never read text from the target app. They use only Dettivo's own last delivery and the input-idle signal.
- `while_dictating` never pauses, ducks or mutes anything while a meeting is capturing: the meeting records the default sink's monitor, so a change there would alter the other side's track.

## Quick commands

- `cargo test -p dettivo-insert -p dettivo-session -p dettivo-cli`
- `dettivo-qa drive insertion_matrix`
- `just build test lint`
