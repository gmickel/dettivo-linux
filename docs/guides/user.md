# User guide

Hold a key, talk, let go, and the words land in the window you were typing in. This guide is the shortest path from an empty Omarchy or Arch machine to that habit and to everything around it (the meeting recorder, the history you can search, the settings you can change in a file or in the app), and each step links to the page that holds the details, so a fact lives in one place.

## Install

```
yay -S dettivo-bin                              # the release build
systemctl --user enable --now dettivod.socket   # the daemon starts on the first request
dettivo setup omarchy                           # the bindings, the bar plugin and the pill on Omarchy
dettivo doctor                                  # what is installed, what is missing, which tier this machine is
```

[docs/install.md](../install.md) names what lands where and how the daemon finds its engines; on plain Hyprland, Sway or Niri, `dettivo setup <compositor>` writes the binding snippet and prints the one include line ([docs/hotkeys.md](../hotkeys.md)). A machine with a Vulkan driver runs the engines on the GPU; one without runs them on the CPU with the smaller default models, and `dettivo doctor` says which tier you are on ([docs/engines.md](../engines.md)).

## First run

The app opens on three screens only when something is missing, and you can skip all three by writing the file yourself ([docs/config.md](../config.md)):

1. **Keys** shows the exact snippet `dettivo setup` prints and confirms a press live ([docs/app.md](../app.md#first-run)).
2. **Models** offers the catalogue with a recommendation per model; a pick downloads it verified and resumable ([docs/models.md](../models.md)).
3. **Try it** dictates into the screen itself and shows where the words went and how long the stop took.

Once the three are done the app opens on Home from then on. Every model download is checked against a checksum pinned in the catalogue and a file that fails is moved aside, never used.

## Dictate

Hold `F9` and talk; release it and the text lands in the focused window. `Super+Ctrl+X` toggles a longer take, `Super+Ctrl+Escape` drops it, `Super+Ctrl+Shift+X` inserts the last transcript again. The pill above your windows shows Listening, Transcribing and then where the words went ([docs/osd.md](../osd.md)); on Omarchy the bar's mark shows the same state and its panel hosts the pill ([docs/omarchy.md](../omarchy.md)). The text goes through the Wayland virtual keyboard when the window accepts it and through a paste when it does not, and the result names the backend that ran ([docs/insertion.md](../insertion.md)). Every take goes to the window that was focused when the key went down, never to Dettivo's own windows and never to a window that changed under you ([docs/dictation.md](../dictation.md)).

## Modes

`[dictation] mode` picks what happens to the engine's words ([docs/polish.md](../polish.md)):

| Mode | What you get |
|---|---|
| `raw` | The engine's words with your replacements and spoken punctuation applied; file names, paths, URLs and versions come back byte for byte. |
| `deterministic_polish` | The Polish pass: grammar fixes, fillers removed, smart punctuation, under a preset per app (email, code, chat, notes, generic) and a style. |
| `enhanced` | Polish, then a rewrite by a language model on this machine (`dettivo llm download` fetches it) or an endpoint you trusted once; when the model does not answer in time the Polish text is inserted and the pill says so. |

`dettivo polish test "<sample>"` shows what each layer makes of a sentence before you change a setting.

## History

Every finished dictation is kept with its text before and after the mode step, where it went, the engine and the take itself, and an audio file you import becomes an item just like it ([docs/history.md](../history.md)). The History route lists them by day, searches by word start, plays the take, exports one item or a range, re-runs a take with another model and deletes an item ([docs/app.md](../app.md#history)); `dettivo history list|search|get|export|import|rerun|delete` does the same from a terminal.

## Meetings

`dettivo meetings start` or the app's Start meeting records your microphone and what the other side says as two tracks on one clock, transcribes live while it runs, finalises the full transcript at stop, names the speakers, writes your notes beside the model's summary, decisions and action items, and exports as txt, md, srt, vtt or json ([docs/meetings.md](../meetings.md)). A headset that vanishes mid-call, a daemon that dies, or a machine that reboots leaves a meeting you can recover from its takes; the first meeting asks you once to acknowledge the disclosure to the other side. The Meetings route shows the list, the live screen with both meters and the detail with its tabs ([docs/app.md](../app.md#meetings)).

## Settings

`~/.config/dettivo/config.toml` is the only source of truth: every key with its type, default and meaning is on [docs/config.md](../config.md), `dettivo config get|set|unset|validate|edit` reads and writes it through the daemon, and the app's Settings route is the same file with a face, eight sections over the same keys ([docs/app.md](../app.md#settings)). A hand edit is picked up without a restart; a value that does not fit is refused with the key named and the file untouched.

## When something fails

- `dettivo doctor` reports the service, the socket, the configuration, every engine with its backend, the insertion chain, the hotkey path, the history store and the pill, and names what is missing.
- `dettivo config validate` names the key and line of a file that does not parse; the daemon keeps running on defaults meanwhile ([docs/config.md](../config.md)).
- `journalctl --user -u dettivod` carries the daemon's log at `[daemon] log_level`; no level logs transcript text or audio ([docs/daemon.md](../daemon.md)).
- The pill's error state says in one sentence what went wrong and what to do next; `dettivo insert target` shows which backend an insertion would use and why the others are out ([docs/insertion.md](../insertion.md)).
- A model that fails verification is moved aside; `dettivo speech status` shows every model's readiness and `dettivo speech download --model <id>` fetches it again ([docs/models.md](../models.md)).
- Keys that do nothing: `dettivo setup <compositor> --check` reports whether the main configuration loads the snippet, and `dettivo hotkeys status` shows the daemon-side backend ([docs/hotkeys.md](../hotkeys.md)).
