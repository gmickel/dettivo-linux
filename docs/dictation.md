# Dictation

A compositor binding can dictate: `dettivo dictation start` records the microphone, `dettivo dictation stop` transcribes the take through the engine and model the session froze at its start, applies the raw text pipeline and hands the text to the desktop, and every transition is published on the event stream so the OSD, the bar and the GUI show the same state. One session runs at a time; a second start is refused, a cancel drops everything, and a device that vanishes ends the session cleanly with a reason the next start does not inherit.

## From a binding

```
dettivo dictation start --language en     # push to talk: bind to a key press
dettivo dictation stop                     # bind to the release
dettivo dictation toggle                   # one binding for both
dettivo dictation cancel                   # drop the take
dettivo dictation status                   # is a session active, what is it doing
dettivo dictation reinsert-last            # hand the last transcript over again
dettivo events --follow --topic dictation.state --topic audio.level
```

`toggle` and `reinsert-last` are Linux additions; the other verbs map one to one to `dictation.start`, `stop`, `cancel` and `status`. `dettivo setup hyprland` (or `sway`, `niri`) writes these bindings for you with the chords from `[hotkeys]`, and on GNOME and KDE the daemon binds the same actions through the portal ([docs/hotkeys.md](hotkeys.md)). `events --follow` subscribes and prints one JSON line per notification (`--json` prints the raw `params` object; `--count N` stops after N events).

## The session

```
idle ──start──▶ recording ──stop──▶ transcribing ──▶ inserting ──▶ idle
                   │                    │
                   └─cancel / device lost / maximum reached ──▶ cancelled | failed ──▶ idle
```

- **start** takes the single session slot before anything else happens, so two concurrent starts never both succeed: the loser gets `CONFLICT` with detail kind `sessionActive`. Before the microphone opens the focused window is recorded as the session's target (`dictation.status.target`), unless the caller passed one with `expected_target_bundle_id` and `expected_target_pid`. The policy is frozen here: provider and model from `[speech]`, language and mode from the request or `[dictation]`, the vocabulary prompt, the replacement table, the maximum duration and the audio retention rule. A `[speech]` change during a session applies to the next one. A model that is not on disk answers `NOT_FOUND` naming it and the `dettivo speech download` command to run; a mode string that is none of `raw`, `polish`, `deterministic_polish` or `enhanced` is `INVALID_PARAMS` naming the four. The `[polish]` rules and the `[llm]` section are frozen here too, so a rule you change during a session applies to the next one.
- **recording** captures 16 kHz mono through PipeWire (a pinned `[audio] input_device` or the default source), meters it onto `audio.level`, and writes the take under `$XDG_STATE_HOME/dettivo/sessions/<job>/`. The session stops itself at `[dictation] max_duration_seconds`. A default source that moves or a pinned device that vanishes ends the session `failed` with the reason; the next start records on the new device.
- **transcribing** sends the take to the engine with the frozen language and vocabulary; a take whose samples' peak stayed under `silence_peak_threshold` is treated as silence and never reaches the engine, so a quiet room does not produce hallucinated filler; the peak is read from the audio itself, so a take shorter than the meter interval still counts. An engine crash fails the session (the reason names the engine, never the audio) and the supervisor restarts the engine on the next use.
- **inserting** runs the mode step (the raw layer, then Polish and the model rewrite when the mode asks for them, [docs/polish.md](polish.md)) and hands the text to the insertion chain ([docs/insertion.md](insertion.md)) in raw mode with the window captured at start as the guard; the result names the backend that typed or pasted it. A focus that moved to another window meanwhile refuses the insertion with `CONFLICT` (checked again right before the backend types), an origin the probe could not name at start sends the text to the clipboard with reason `origin_unverified`, a backend that may have typed a prefix ends the insertion as `failed` with reason `partial_delivery` and the text is never typed twice, and when no backend is available it reports `failed` with the reason; the transcript is kept either way, and `dictation.stop` reports the outcome in its `insertion` block.
- **idle** is published after the item is in the history store ([docs/history.md](history.md)) with the transcript, the insertion result, the app, the engine and model and, when `[history] keep_audio` is on, the take moved into the item's directory; the session keeps the last transcript for `reinsert-last` too, and after a restart `reinsert-last` reads the newest item from the store. A store that fails puts `history: <why>` on this state's `reason` and the insertion stands.

`dictation.status` reports `is_active`, the job (`listening with whisper/large-v3-turbo`, progress as a fraction of the maximum duration) and the `target` captured at start; `system.health` reports `recording_state = dictation` and one active job while a session runs.

## The modes

`raw` returns the engine's text with three deterministic passes: whole-word, case-insensitive replacements from `[dictation] replacements` (`replacements = { teh = "the" }`), spoken punctuation when `spoken_punctuation` is on (`comma`, `period` or `full stop`, `question mark`, `exclamation mark`, `colon`, `semicolon`, `new line`, `new paragraph`, `open quote` and `close quote`, `open paren` and `close paren`, `dash`, `ellipsis`), and whitespace discipline with a capital after a sentence end. File names, paths, URLs, email addresses, versions, dotted identifiers and backticked spans pass through all three byte for byte: a dot with no whitespace around it is part of the token (`index.ts`, `example.com`, `v1.2.3`, `foo.bar()`), a replacement never rewrites inside one (`example` stays as it is in `https://example.com`), and the sentence end is the dot that whitespace follows, so "open index.ts. Then run it" keeps both. `[dictation] protect_tokens = false` makes every dot a sentence end again. `deterministic_polish` runs that result through the Polish pass — fillers, casing, punctuation, and the preset's repairs for spoken paths, lists and abbreviations — with no model involved. `enhanced` sends the Polish result to a language model and keeps the Polish text whenever the model is slow, missing or wrong, with a notice saying which. Both are [docs/polish.md](polish.md); `polish` is the contract's alias for `deterministic_polish`.

## The event stream

`events.subscribe { topics, buffer }` answers a `subscription_id`; the daemon then sends `events.notify` server notifications on the same connection with `subscription_id`, `topic`, `timestamp` and `payload`, until `events.unsubscribe` or the connection closes. Unknown topics are `INVALID_PARAMS`. Each connection holds up to 256 notifications for a reader that has fallen behind (the granted `buffer` is always 256 until per-subscription depths are enforced); beyond that, events are dropped and counted, and `events.overflow` with the count is delivered as soon as the reader has room again.

| Topic | Payload |
|---|---|
| `dictation.state` | `job_id`, `state` (`recording`, `transcribing`, `inserting`, `idle`, `cancelled`, `failed`), `previous_state`, `reason`; the transition into `idle` from `inserting` also carries `insertion` (`outcome`, `method`, `backend`, `target_app`, `reason`), `first_words`, `timings` (`capture_ms`, `transcribe_ms`, `insert_ms`: where the time after the key release went) and, for the Polish modes, `mode`, `notice` (`kind`, `reason`) and `policy_hash`, so the pill names where the words went without a second request and the QA rig measures the path ([docs/qa.md](qa.md)) |
| `audio.level` | `rms`, `peak` (0 to 1), `source` (`microphone`, or `system` for a meeting's system track) |
| `engine.state` | `binary`, `state` (`spawned`, `loaded`, `unloaded`, `crashed`, `degraded`), `model`, `backend`, `reason` |
| `model.download` | provider, model, state, bytes done and total, error |
| `meeting.state` | the meeting lifecycle with its bounded live metadata ([docs/meetings.md](meetings.md)) |
| `job.progress`, `meeting.segment` | `job.progress` is in [docs/history.md](history.md); `meeting.segment` is in [docs/meetings.md](meetings.md) |

Payloads carry ids and states; the one exception is `first_words` on the completion transition, the transcript's first sentence bounded to 72 characters, which the pill shows for a moment. The daemon's logs never carry transcript text.

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[dictation] language` | `"auto"` | The language the engine is told; `dictation.start` may override it. |
| `[dictation] mode` | `"raw"` | The mode; `polish` is gated on the polish spec. |
| `[dictation] vocabulary` | `[]` | Words the engine is primed with. |
| `[dictation] replacements` | `{}` | Whole-word replacements applied after recognition. |
| `[dictation] spoken_punctuation` | `true` | Spoken punctuation becomes marks. |
| `[dictation] protect_tokens` | `true` | File names, paths, URLs, addresses, versions and dotted identifiers pass through untouched. |
| `[dictation] max_duration_seconds` | `300` | The longest take. |
| `[dictation] silence_peak_threshold` | `0.01` | Under this peak the take is silence. |
| `[history] keep_audio` | `true` | Keep the take with the stored item. |
| `[audio] input_device` | `""` | A pinned PipeWire node; empty follows the default. |

## Testing without a microphone

`DETTIVO_MOCK_MIC=<file.wav>` (with `DETTIVO_QA_MODE=1`, since every QA hook needs QA mode) makes every session capture the fixture instead of PipeWire, at real time, through the same event stream; the daemon's dictation tests dictate `jfk.wav` through the real Whisper engine this way, and the QA rig sets it for drives.
