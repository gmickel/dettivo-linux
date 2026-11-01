# Dictation

A compositor binding can dictate: `dettivo dictation start` records the microphone, `dettivo dictation stop` transcribes the take through the engine and model the session froze at its start, applies the raw text pipeline and hands the text to the desktop, and every transition is published on the event stream so the OSD, the bar and the GUI show the same state. One session runs at a time; a second start is refused, a cancel drops everything, and a device that vanishes ends the session cleanly with a reason the next start does not inherit.

## From a binding

```
dettivo dictation start --language en     # push to talk: bind to a key press
dettivo dictation stop                     # bind to the release
dettivo dictation stop --print             # finish it and print the text instead of inserting it
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
- **transcribing** passes the take through the speech gate, then sends it to the engine with the frozen language and vocabulary (see [The speech gate](#the-speech-gate)). A take the gate finds silent never reaches the engine and ends `cancelled` with `reason` `silence: level` or `silence: voice`, and the pill says "Nothing heard". An engine crash fails the session (the reason names the engine, never the audio) and the supervisor restarts the engine on the next use.
- **inserting** runs the mode step (the raw layer, then Polish and the model rewrite when the mode asks for them, [docs/polish.md](polish.md)) and hands the text to the insertion chain ([docs/insertion.md](insertion.md)) in raw mode with the window captured at start as the guard; the result names the backend that typed or pasted it. A focus that moved to another window meanwhile refuses the insertion with `CONFLICT` (checked again right before the backend types), an origin the probe could not name at start sends the text to the clipboard with reason `origin_unverified`, a backend that may have typed a prefix ends the insertion as `failed` with reason `partial_delivery` and the text is never typed twice, and when no backend is available it reports `failed` with the reason; the transcript is kept either way, and `dictation.stop` reports the outcome in its `insertion` block.
- **stop --print** finishes the take and runs the mode step like `stop`, then prints the text on standard output and inserts nothing; the history item records the insertion as `printed` and the pill shows the take as done with no window named. It waits up to 60 s for the transcription. It exits 0 with text and 6 (`NoSpeech`) when no speech was heard, so a script or a terminal agent can read a dictation with `text=$(dettivo dictation stop --print)`. `--json` prints the whole result with the text in `text`.
- **A printing stop that comes too late**, once the take's text was already handed to the inserter by another stop, is refused with `CONFLICT` kind `sessionDelivering` instead of printing a take that was typed.
- **A silent take** answers `stop` and `stop --print` alike with `CONFLICT` kind `noSpeech` ("no speech was heard"), and the CLI exits 6.
- **idle** is published after the item is in the history store ([docs/history.md](history.md)) with the transcript, the insertion result, the app, the engine and model and, when `[history] keep_audio` is on, the take moved into the item's directory; the session keeps the last transcript for `reinsert-last` too, and after a restart `reinsert-last` reads the newest item from the store. A store that fails puts `history: <why>` on this state's `reason` and the insertion stands.

`dictation.status` reports `is_active`, the job (`listening with whisper/large-v3-turbo`, progress as a fraction of the maximum duration) and the `target` captured at start; `system.health` reports `recording_state = dictation` and one active job while a session runs.

## The speech gate

The gate keeps an empty or noisy take from pasting the filler an engine invents for silence ([ADR 0081](adr/0081-a-dictation-take-passes-a-speech-gate-and-its-text-passes-output-guards.md)). The stop ends the capture at once, so the take holds what you recorded up to the release plus at most one capture quantum (about 21 ms) that PipeWire may deliver before the stop runs; the stop cue plays after that and never counts ([ADR 0082](adr/0082-a-take-ends-at-the-stop-within-one-capture-quantum.md)). The gate never trims a take that passes.

1. **Level.** The loudest 30 ms window of the take, with its DC offset removed, must reach `[dictation] silence_dbfs` (-50 dBFS). A click or keyboard thump stays under it, and a quiet word at -45 dBFS passes. A take shorter than 30 ms is silence.
2. **Voice.** When `whisper/silero-vad` is downloaded, the Silero model looks for speech in the take on the CPU (threshold 0.4, at least 160 ms of speech), so a fan, a music bed or breathing is silence while a whisper and a single "Ok" are not. `speech.models.download` fetches it with any speech model, and `dettivo speech download --model silero-vad` fetches it alone. Without it the level decides alone, and `dettivo doctor` names the model and that command on its `voice gate` row. Right after the daemon starts, a model on disk that has not been hashed yet shows as `unverified` on that row; the daemon checks it before the first take that uses it, so no download is needed.

A take that passes reaches the engine at least 1.5 s long, and a Parakeet take gets 400 ms of trailing silence so the engine reaches its last word. When the padded take comes back with no text, the session recognizes it once more as captured, because Parakeet v3 empties some short takes once silence is appended. The engine's text then loses, per segment, a decoder loop (the words before it stay), the silence hallucinations and blank markers the meeting filter drops (`thanks for watching`, `[BLANK_AUDIO]` and their kin), and an echo of the whole vocabulary prompt when the prompt has two or more terms. Dictating the one listed term still pastes it. A take whose text ends up empty completes without an insertion, and the pill says "Nothing heard".

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

## Vocabulary

A vocabulary teaches Dettivo the names and jargon you dictate. Each entry is a term, and an entry can also say how the engine tends to mishear it:

```toml
[dictation]
vocabulary = [
  "Dettivo",
  { term = "Clef", sounds_like = ["clay", "cleff"] },
  { term = "SapienXT", sounds_like = ["sapient x t"] },
]
```

- **Whisper** is primed with the terms only, joined by commas and closed with a full stop, up to 500 characters of whole terms ([docs/engines.md](engines.md)). The `sounds_like` spellings never reach the engine, so it never learns a misspelling.
- **Enhanced** gets each term with its spellings in the last line of its system prompt, so the rewrite can correct `clay` to `Clef` when the sentence means the product: `Vocabulary (spell these exactly when the speaker means them): Clef (may be transcribed as clay, cleff); SapienXT (may be transcribed as sapient x t); Dettivo`. The first 50 entries go in.
- **Raw and Polish** take nothing from the spellings. A `sounds_like` spelling never rewrites text by itself; `[dictation] replacements` is the tool for a substitution you always want.

A vocabulary holds at most 500 entries, a term at most 64 characters and 16 `sounds_like` spellings. A file, `dettivo config set` or Settings that breaks a limit is refused with the entry named. A term that repeats an earlier one in another case is dropped as the file loads, and the first spelling stays. `dettivo-qa bench vocabulary` measures whether a vocabulary helps ([docs/qa.md](qa.md#vocabulary-term-recall)).

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[dictation] language` | `"auto"` | The language the engine is told; `dictation.start` may override it. |
| `[dictation] mode` | `"raw"` | The mode; `polish` is gated on the polish spec. |
| `[dictation] vocabulary` | `[]` | Terms the engine is primed with. An entry is a term, or a `{ term, sounds_like }` table naming how the term tends to be misheard ([Vocabulary](#vocabulary)). |
| `[dictation] replacements` | `{}` | Whole-word replacements applied after recognition. |
| `[dictation] spoken_punctuation` | `true` | Spoken punctuation becomes marks. |
| `[dictation] protect_tokens` | `true` | File names, paths, URLs, addresses, versions and dotted identifiers pass through untouched. |
| `[dictation] max_duration_seconds` | `300` | The longest take. |
| `[dictation] silence_dbfs` | `-50.0` | Under this level, over the loudest 30 ms, the take is silence. |
| `[history] keep_audio` | `true` | Keep the take with the stored item. |
| `[dictation] join_continuations` | `true` | A take into the window the previous one landed in, within 30 s and with no input in between, continues it ([docs/insertion.md](insertion.md#consecutive-dictations-join)). |
| `[audio] input_device` | `""` | A pinned PipeWire node; empty follows the default. |
| `[audio] while_dictating` | `"nothing"` | What a take does to playing media (see [Media while dictating](#media-while-dictating)). |

If your `config.toml` still sets `[dictation] silence_peak_threshold`, it keeps loading and the daemon ignores that value, because `silence_dbfs` replaced it ([ADR 0081](adr/0081-a-dictation-take-passes-a-speech-gate-and-its-text-passes-output-guards.md)). Delete the line when convenient. Its value does not carry over, since the old key measured the single loudest sample and `silence_dbfs` measures the loudest 30 ms window in dBFS; [docs/config.md](config.md) has the full migration note.

## Media while dictating

`[audio] while_dictating` decides what a dictation does to playing music or video ([ADR 0091](adr/0091-delivery-restores-the-whole-clipboard-joins-continuations-and-ducks-media.md)):

| Value | While the take records | After it |
|---|---|---|
| `"nothing"` | Media plays on. | Nothing. |
| `"pause"` | The playing MPRIS players pause. | The players Dettivo paused resume. |
| `"duck"` | The default output falls to 20 % volume along a smoothstep curve over about 200 ms; a quieter output stays as it is. | The volume rises back the same way. |
| `"mute"` | The default output mutes. | It unmutes. |

Media changes only once a take has recorded for 300 ms, the hold key's minimum hold, so a tap the hold key throws away never touches it. Dettivo puts back only what it changed. When you change the volume during the duck ramp, the ramp stops there, and a volume or a mute you set during the take stays after it. The volume goes through `wpctl` on `@DEFAULT_AUDIO_SINK@`, the pause through MPRIS on the session bus, all on a thread of its own, so the microphone never waits for it. Nothing changes during a meeting: a meeting and a dictation never run at once, and a take's end waits until media is back before its slot frees. Dettivo changes and restores the sink that was the default when the take started, so switching outputs mid-take leaves the new one alone, and volumes move in hundredths, the precision `wpctl` reports, so a change of one point is yours and stays. `[hotkeys] pause_media = true` with `while_dictating` left at `"nothing"` reads as `"pause"`.

## Testing without a microphone

`DETTIVO_MOCK_MIC=<file.wav>` (with `DETTIVO_QA_MODE=1`, since every QA hook needs QA mode) makes every session capture the fixture instead of PipeWire, at real time, through the same event stream; the daemon's dictation tests dictate `jfk.wav` through the real Whisper engine this way, and the QA rig sets it for drives.
