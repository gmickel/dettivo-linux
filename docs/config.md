# Configuration

`config.toml` is the source of truth for every setting (ADR 0009). The daemon reads it at start and again whenever it changes, the `dettivo` command reads and writes every key, and the settings routes in the app edit the same file. A machine provisioned with this file and a model directory needs no first-run screens.

## Using Settings

Settings opens in Simple mode with everyday recording, model, polish and retention choices. **Show advanced settings** reveals paths, timeouts, pipeline tuning, connection details and troubleshooting. Switching views only changes presentation and keeps every configured value. The mode applies across settings pages for this app session.

Language and model menus name their choices. Automatic language detection and inherited meeting/analysis models have explicit labels. A configured value missing from a menu stays visible as custom/unavailable until you choose another value. The language menu lists Whisper language names; support depends on the selected speech model. Custom codes from the file remain visible.

**Vocabulary** has its own page. Enter a term or phrase, choose **Add term**, and use **Remove** beside an entry to delete it. Entries containing commas remain single terms. Polish offers separate switches for grammar, fillers and punctuation.

Each editable setting has **Reset**. Its tooltip shows the canonical default. Reset removes the file override through the daemon, which validates the result, preserves unrelated values and comments, and refreshes the effective value. Empty path defaults follow the system's configured directories. Resetting the dictation model selects the canonical provider and model together. Environment-controlled settings remain locked. Errors stay beside the affected control. Paths and listener settings that require a restart retain the behavior described below.

Advanced mode also shows the generated shortcut snippet and the TOML preview. **Apply shortcuts**, the setup status and **Copy include line** remain available in Simple mode. **Open in editor** opens the same file the UI edits.

## Where it lives

| What | Path | Override |
|---|---|---|
| Configuration | `$XDG_CONFIG_HOME/dettivo/config.toml` (`~/.config/dettivo/config.toml`) | `DETTIVO_CONFIG=<path>` or `dettivod --config <path>` |
| Runtime state (not configuration) | `$XDG_STATE_HOME/dettivo/state.toml` | none |
| Data (history, models) | `$XDG_DATA_HOME/dettivo` | `DETTIVO_DATA_DIR=<dir>`, or `paths.data_dir` |
| History database | `<data_dir>/dettivo.db` | `history.db_path` |
| Transfer staging and exports | `$XDG_CACHE_HOME/dettivo` | none |
| Socket | `$XDG_RUNTIME_DIR/dettivo/dettivo.sock` | `DETTIVO_IPC_SOCKET=<path>`, `dettivod --socket`, or `ipc.socket`; a systemd socket unit wins over all of them |
| Token file (`peer_token` mode) | `$XDG_CONFIG_HOME/dettivo/ipc.token` | `ipc.token_file` |

`dettivo config path` prints every resolved location. Precedence for a key is environment, then file, then default; `dettivo config get <key>` shows the value in force and which of the three it came from.

A changed value reaches the daemon on its next reload of the file, and every key applies from that moment, with four exceptions that name a resource the running daemon already holds: `ipc.socket`, `paths.data_dir`, `history.db_path` and the `[rest]` listener keys apply at the next daemon start. Until then `dettivo config get` shows the new value with its source while the daemon keeps the socket, the database and the listener it opened; `dettivo doctor` reports the paths in use.

## Working with the file

```
dettivo config print-default > /tmp/dettivo-config.example.toml  # inspect the commented defaults
dettivo config get                       # every key, value and source
dettivo config get daemon.log_level
dettivo config set daemon.log_level debug
dettivo config unset daemon.log_level    # back to the default
dettivo config validate                  # names the key and line of a problem
dettivo config edit                      # $VISUAL / $EDITOR, then validate
```

The package ships the same commented reference at `/usr/share/doc/dettivo/config.example.toml`, generated offline from the canonical defaults. Compare it with your existing file and copy only the settings you want to change. The reference explains units, special empty/zero values, providers, privacy and restart behavior.

Writes go through the daemon and keep your comments and ordering. A value typed on the command line is coerced to the key's declared type (`5000` becomes an integer, `yes` a boolean, `foot, kitty` a list of strings, `foot=ctrl+shift+v` a table of strings); a value that does not fit, or a key that does not exist, is refused with the key named and the file untouched. The daemon never writes a file it could not read back.

A file that does not parse never puts the daemon on the defaults: the values the file asked for, the `peer_token` requirement and the retention among them, are what run. An edit that breaks the file while the daemon runs leaves the last valid values in force and says so: `system.health` reports `ok = false`, `dettivo config validate` names the key and line, and `journalctl --user -u dettivod` carries the key and line (never the value); fixing the file is picked up without a restart. A file that is already broken when the daemon starts stops the start, with the key and line on stderr and in the journal; `dettivo config edit` opens it without a daemon, and the next start reads the fix. A missing file is the defaults.

Runtime state that is not configuration (window geometry, acknowledgements, first-run progress, the daemon's last start) lives in `state.toml` and is never written into `config.toml`.

## Keys

### `[daemon]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `log_level` | `error` \| `warn` \| `info` \| `debug` \| `trace` | `info` | Lowest level written to the journal; a change applies on reload. `RUST_LOG` overrides it for one run. No level logs transcript text, audio or prompts. |
| `shutdown_timeout_ms` | integer | `5000` | How long the whole shutdown may take from the signal: the connection drain, the socket release and the stops of the REST shim, the hotkeys, the downloads and the engines share this one budget, and a step still running at its end is left behind and named in the journal. The service unit's `TimeoutStopSec=10` caps it under systemd. |

### `[ipc]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `auth_mode` | `peer` \| `peer_token` | `peer` | `peer`: any process of the same user may connect (`SO_PEERCRED`). `peer_token`: the same plus a shared token on every request, read from `DETTIVO_IPC_TOKEN`, then the Secret Service item (service `dettivo`, key `ipc-token`, via `secret-tool`), then `token_file`. `system.capabilities.auth.ipc_mode` reports the mode in force. |
| `socket` | path | `""` | Socket path; empty means `$XDG_RUNTIME_DIR/dettivo/dettivo.sock`. `DETTIVO_IPC_SOCKET` and the systemd socket unit override it. Applies at the next daemon start. |
| `token_file` | path | `""` | Token file for `peer_token` mode, mode `0600` or tighter; empty means `$XDG_CONFIG_HOME/dettivo/ipc.token`. A file other users can read is ignored. |
| `max_line_bytes` | integer | `1048576` | Longest request line the daemon accepts. A longer line is answered with `INVALID_PARAMS` and the connection stays open. |

### `[paths]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `data_dir` | path | `""` | History and settings data; empty means `$XDG_DATA_HOME/dettivo`. `DETTIVO_DATA_DIR` overrides it. Applies at the next daemon start. |
| `models_dir` | path | `""` | Model files; empty means `<data_dir>/models`. |

### `[engines]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `directory` | path | `""` | Directory searched first for engine binaries (`dettivo-engine-whisper` and the others). Empty means the built-in order: `/usr/lib/dettivo/engines-cuda` (the optional CUDA diarization drop-in), then `/usr/lib/dettivo/engines` (where the package installs them), then the daemon's own directory, then `PATH`; a set directory goes in front of that order ([docs/install.md](install.md)). `speech.engines` reports which directory answered. |
| `stt_idle_seconds` | integer | `300` | Seconds a speech engine stays warm after its last request; then it is unloaded and its process ends, which returns GPU memory. |
| `llm_idle_seconds` | integer | `600` | The same for the language model engine (`dettivo-engine-llm`); `llm.engine.status` reports it and the memory the engine holds until then. |
| `whisper.backend` | `auto` \| `vulkan` \| `cpu` | `"auto"` | `[engines.whisper]`: the compute backend the Whisper engine loads on. `auto` takes Vulkan when the build carries it, a device is installed and the model loads there, else the CPU with the reason in `speech.engines`; `vulkan` fails the load instead of falling back; `cpu` never touches the GPU. `DETTIVO_FORCE_CPU=1` overrides both engines. |
| `parakeet.backend` | `auto` \| `vulkan` \| `cpu` | `"auto"` | `[engines.parakeet]`: the same for the Parakeet engine ([docs/engines.md](engines.md)). |
| `llm.backend` | `auto` \| `vulkan` \| `cpu` | `"auto"` | `[engines.llm]`: the same for the language model engine; `auto` falls back to the CPU with the reason when the model does not fit the device's free memory (ADR 0026). |
| `llm.context_length` | integer | `4096` | `[engines.llm]`: the context length in tokens the model is loaded with. |
| `llm.max_tokens` | integer | `1024` | `[engines.llm]`: the most tokens one Enhanced rewrite may generate. |
| `diarize.backend` | `auto` \| `cpu` \| `cuda` | `"auto"` | `[engines.diarize]` selects the ONNX Runtime provider. `auto` selects CUDA when its provider, CUDA 13/cuDNN 9 dependencies and device checks pass, else CPU with `fallback_reason`; `cuda` fails the load naming the missing dependency; `cpu` never touches the GPU. `DETTIVO_FORCE_CPU=1` overrides this choice. Settings / Models edits the same key ([docs/engines.md](engines.md), ADR 0057). |
| `diarize.threads` | integer | `0` | `[engines.diarize]` sets threads per model; 0 is the core count capped at four ([docs/meetings.md](meetings.md)). |

### `[speech]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `provider` | string | `"whisper"` | The speech provider: `whisper` or `parakeet`. |
| `model` | string | `"large-v3-turbo"` | The model id, a directory under `<models>/<provider>/` (see [docs/engines.md](engines.md)). A session freezes provider and model at start; a change applies to the next session. |
| `meeting_model` | string | `""` | Whisper model for meetings, independent of the dictation provider. Empty inherits the dictation model and provider; Parakeet requires an explicit Whisper meeting model. |
| `parakeet_model_id` | string | `"parakeet-v3"` | The Parakeet model `speech.selection.set { provider: "parakeet" }` (and `dettivo speech selection set --provider parakeet`) switches `model` to when the current model is not a Parakeet model; `parakeet-v2` (English) or `parakeet-v3` (25 European languages). |

### `[models]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max_concurrent_downloads` | integer | `1` | How many model downloads may run at once; further requests wait. |
| `verify_on_start` | bool | `true` | Re-hash every model on disk that has no verified manifest when the daemon starts, before the preload; a mismatch moves the file to `<models>/quarantine/`. Off, each load hashes an unverified model first; no engine opens a catalogue model that did not verify. |
| `catalogue_file` | path | `""` | A catalogue TOML to use instead of the built-in one, for mirrors and tests ([docs/models.md](models.md)). |
### `[audio]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `input_device` | string | `""` | PipeWire node name to capture from (`dettivo doctor` and `audio.devices` list them). Empty follows the default source, and keeps following it when the default changes. A pinned device that disappears ends a dictation with the device named; a meeting restarts its microphone on the default and records the gap ([docs/meetings.md](meetings.md)). |
| `level_interval_ms` | integer | `50` | How often a level sample (RMS and peak, 0 to 1) is published for the meters. |
| `system_source` | string | `"default_monitor"` | A meeting's system track: `default_monitor` captures the monitor of the default sink and follows the default when it changes; a sink's node name (`audio.devices` lists sinks as `sink_monitor`) pins that sink's monitor. |
### `[insert]`

Text insertion (ADR 0007, [docs/insertion.md](insertion.md)).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `backend` | `auto` \| `virtual_keyboard` \| `libei` \| `ydotool` \| `xdotool` \| `clipboard_paste` \| `clipboard` | `auto` | `auto` runs the chain in that order. A backend name pins it; an unavailable pin fails with its reason and never falls back. |
| `paste_keys` | table of strings | `{}` | The paste keystroke per app id for the clipboard path: `ctrl+v`, `ctrl+shift+v` or `shift+insert`. `dettivo config set insert.paste_keys "foot=ctrl+shift+v, Code=ctrl+v"`. |
| `terminal_app_ids` | list of strings | `foot`, `footclient`, `com.mitchellh.ghostty`, `ghostty`, `Alacritty`, `alacritty`, `kitty`, `org.wezfurlong.wezterm`, `wezterm`, `xterm`, `org.gnome.Console`, `org.gnome.Terminal`, `org.kde.konsole` | App ids that are terminals and take `ctrl+shift+v`. `dettivo config set insert.terminal_app_ids "foot, kitty"`. |
| `self_app_ids` | list of strings | `dettivo`, `dettivo-app`, `dettivo-osd`, `dettivo-sheet` | App ids of Dettivo's own windows; insertion into one of them fails with `target_is_self` and never pastes. |
| `inter_key_delay_ms` | integer | `2` | Pause between typed keys. |
| `restore_clipboard` | boolean | `true` | Put the previous clipboard back after a clipboard insertion. |
| `clipboard_restore_delay_ms` | integer | `300` | How long the restore waits for the target to take the paste. A clipboard you changed meanwhile is left alone. |
| `undo_window_ms` | integer | `5000` | How long after an insertion `dettivo insert undo` may take it back. |

### `[dictation]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `language` | string | `"auto"` | The language the engine is told, or `auto` to detect. `dictation.start` may override it per session. |
| `mode` | string | `"raw"` | `raw`, `deterministic_polish` (the contract's `polish` is an accepted alias) or `enhanced` ([docs/polish.md](polish.md)); any other string is `INVALID_PARAMS` naming the four. |
| `vocabulary` | array of strings | `[]` | Words the engine is primed with (names, products, jargon). |
| `replacements` | inline table | `{}` | Whole-word, case-insensitive replacements applied after recognition: `replacements = { teh = "the" }`. |
| `spoken_punctuation` | boolean | `true` | `comma`, `period`, `question mark`, `new line` and friends become marks. |
| `protect_tokens` | boolean | `true` | File names, paths, URLs, email addresses, versions, dotted identifiers and backticked spans pass through the raw layer byte for byte: no sentence end on a dot inside one, no replacement inside one ([docs/dictation.md](dictation.md)). `false` makes every dot a sentence end. |
| `max_duration_seconds` | integer | `300` | The longest take; the session stops itself there. |
| `silence_peak_threshold` | float | `0.01` | A take whose peak level stays under this is treated as silence and produces an empty transcript. 0 to 1. |

### `[history]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `keep_audio` | boolean | `true` | Keep the take of every finished dictation beside its item under `<data_dir>/dictations/<id>/`, so it can be re-run with another model ([docs/history.md](history.md)). Meeting audio is always kept until the meeting is deleted. |
| `audio_retention_days` | integer | `30` | Days after which the hourly sweep removes retained audio; `0` keeps it. |
| `max_items` | integer | `0` | Items kept; the sweep removes the oldest beyond this count. `0` keeps every item. |
| `artifacts` | `keep` \| `audio_only` \| `none` | `"keep"` | What lives beside the database and what a delete removes: `keep` writes audio and `metadata.json` and a delete removes the item directory; `audio_only` writes the audio alone; `none` writes nothing beside the row. |
| `db_path` | path | `""` | The SQLite database; empty means `<data_dir>/dettivo.db`. Applies at the next daemon start. |
| `max_import_seconds` | integer | `14400` | The longest audio file `dettivo history import` accepts (four hours). A longer file is refused after the probe, before anything is decoded, with the limit named. |

### `[transcribe]`

The chunked pipeline every import and re-run goes through (ADR 0022, [docs/history.md](history.md)); the default window is 30 seconds so automatic language detection can follow language changes.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `chunk_seconds` | integer | `30` | Maximum seconds of audio per recognition window; conservative silence trimming can shorten it. Must exceed `overlap_seconds` plus `safety_margin_seconds`; a file, a `config set` or an edit that breaks that is refused naming this key. |
| `overlap_seconds` | integer | `2` | Seconds two consecutive chunks share; the merger reconciles the overlap by timestamp. |
| `safety_margin_seconds` | integer | `5` | Seconds before a chunk's nominal end inside which the cut moves to the quietest point, so a chunk rarely ends mid-word. |
| `silence_rms_floor` | float | `0.0065` | RMS (0 to 1) under which a chunk counts as silence, judged over every 200 ms frame of it: a chunk whose frames all stay under the floor never reaches the engine and adds no text, so Whisper cannot invent any, and one short utterance in a long silence still reaches it. Before recognition, quiet edges below one tenth of this floor are trimmed with 400 ms padding; timestamps retain their original positions. |
| `filler_filter` | boolean | `true` | Drop the known filler hallucinations (`thanks for watching` and its kin) before the text is stored. |

### `[meetings]`

The two-stream capture, its checkpoint and what stays afterwards (ADR 0027), and the live transcription tuning (ADR 0031); [docs/meetings.md](meetings.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `keep_audio` | boolean | `true` | Keep `microphone.wav` (one per take), `system.wav` and their sidecars under `<data_dir>/meetings/<id>/` after the meeting; off removes the takes at stop. |
| `checkpoint_interval_seconds` | integer | `15` | Seconds between two writes of `live-checkpoint.json` while recording, each with a flush of both takes; a daemon that dies mid-meeting recovers the meeting from the last one. |
| `artifacts` | `keep` \| `audio_only` \| `none` | `"keep"` | What lives in the meeting directory once the meeting stopped: `keep` writes `metadata.json` beside the takes, the sidecars and `journal.jsonl`; `audio_only` skips `metadata.json`; `none` removes the directory. `meetings.delete` takes the contract's `artifact_policy` instead. |
| `live` | boolean | `true` | Transcribe while the meeting records (ADR 0031): windows of both tracks go through the selected engine and arrive as `meeting.segment` events; off runs the finalisation alone when the meeting stops. |
| `live_window_ms` | integer | `3000` | The longest live window sent to the engine; a window grows towards it while the engine catches up, and audio further than two ticks past a full window is skipped with a journal line. Must hold `live_overlap_ms + live_tick_ms`, and both must be positive; a value that does not is refused at validation naming this key. |
| `live_overlap_ms` | integer | `450` | Milliseconds two consecutive live windows share, so a word cut by one window is whole in the next; the merger reconciles the overlap by timestamp. |
| `live_tick_ms` | integer | `900` | Milliseconds of new audio that cut the next live window. |
| `boundary_merge_gap_ms` | integer | `1200` | A live segment that ends this far before the newest audio is final; nearer ones stay provisional and the next window replaces them whole. |
| `cross_source_padding_ms` | integer | `800` | A microphone segment that is a filler (`yeah`, `okay`, four characters or less) within this many milliseconds of a remote segment at least twice as long is the speakers heard through the microphone, and is dropped. |
| `speech_floor_rms` | float | `0.0065` | RMS (0 to 1) a live window must reach to be sent to the engine while nobody spoke; 70 % of it (the macOS continuation floor) keeps a window going right after speech. A window under the floor never reaches the engine. |
| `delete_artifact_policy` | `transcript_only` \| `transcript_and_audio` \| `all` | `"all"` | The artifact policy `dettivo meetings delete` and the app use when none is named: `transcript_only` clears the texts, segments and analysis and keeps the notes and the audio; `transcript_and_audio` removes the takes too and keeps the row's facts and the notes; `all` removes the row and the directory ([docs/meetings.md](meetings.md#delete)). |

### `[meetings.analysis]`

The summary, decisions and action items a meeting gets once it finalised (ADR 0036, [docs/meetings.md](meetings.md#analysis)).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `auto` | boolean | `true` | Run the analysis after every finalisation (and after an import into a meeting); off leaves it to `dettivo meetings analyze <id>`. `meetings.start` and `transcripts.import` take `analyze` to decide per meeting. |
| `timeout_ms` | integer | `60000` | The whole analysis, every part and the merge, must finish within this; then it is `failed` with `provider_unavailable` and `meetings.analyze` runs it again. |
| `chunk_chars` | integer | `12000` | Characters of transcript per model call. A longer transcript is cut on segment boundaries and each part analysed on its own; a part whose answer the model cut off is halved and analysed in two (up to four levels), the parts' decisions and action items are joined in code, and the model combines their summaries in calls of at most this many characters, retried on smaller groups when a call does not fit (ADR 0062). On the local engine's 4096-token context a part of 12000 characters can overflow; a smaller value avoids the wasted call. |
| `provider` | string | `""` | The provider the analysis asks: `local`, `ollama`, `openai_compatible` or `auto`; empty follows `[llm] provider`. `[llm] analysis_model` names the local model. |

### `[meetings.diarization]`

The post-meeting speaker pass (ADR 0035): the system track (the whole microphone track of a room-audio meeting) goes through `dettivo-engine-diarize` once the finalisation completes, and every remote segment gets a speaker under the rule below; [docs/meetings.md](meetings.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | boolean | `true` | The pass is available: `meetings.diarize` and the automatic run. Off answers `meetings.diarize` with `CONFLICT` and runs nothing. |
| `auto` | boolean | `true` | Run the pass by itself when a meeting's finalisation completes; off leaves it to `dettivo meetings diarize <id>`. `meetings.start` and a meeting import take `diarize` to override it for one meeting. |
| `model` | string | `"diarization-en"` | The calibrated English model set under `<models>/diarize/` (`dettivo speech download --provider diarize --model diarization-en`). The original `diarization` set remains a separate catalogue identity. |
| `pause_ms` | integer | `250` | Each sentence takes the speaker who holds most of it ([ADR 0072](adr/0072-every-remote-line-takes-the-speaker-of-its-sentence.md)). Where the engine aligned words, a gap of at least this many milliseconds between two words also ends a sentence, but only where the diarized speaker differs across it. Whisper aligns no words, so this key acts on Parakeet transcripts. |
| `nearest_turn_ms` | integer | `10000` | A sentence no speaker turn overlaps takes the nearest turn within this many milliseconds, with `speaker_confidence` 0. Beyond it the line stays unlabelled. |
| `min_speaker_share` | float | `0.0` | The least share of a sentence's diarized speech the winning speaker must hold, 0 to 1. At 0 every sentence a turn overlaps gets a name. At 0.6 a sentence two voices share evenly stays unlabelled, which trades most of the gain in named lines for fewer wrong names. |
| `max_speakers` | integer | `0` | The speaker count the clustering is told; 0 lets it decide. `meetings.start` and `meetings.diarize` take `expected_speakers` for one meeting. |
| `clustering_threshold` | float | `0.6` | The average-linkage cosine distance threshold when the count is not fixed; smaller finds more initial clusters. Reliable embeddings train the clusters, and all embeddings are assigned to retained centroids. A finite number, 0 or more. |

**Migrating from the coverage and share rule.** `min_coverage` is retired. A file that still sets it loads, and the daemon ignores the value, so delete the line when convenient. `min_speaker_share` keeps its name and now applies to a sentence instead of a segment. A file that sets it to the old default of 0.6 keeps leaving mixed lines blank, and removing the line (or setting 0) labels them. Run `dettivo meetings diarize <id>` to relabel a meeting the old rule left with blank lines.

Existing explicit model and threshold settings stay in force. The calibrated automatic algorithm replaces complete-linkage threshold clustering; a previously tuned threshold should be evaluated again. Download `diarization-en` and select it explicitly to migrate an existing configuration. Its files occupy a separate directory, so the original model set is retained. A positive `max_speakers` or per-meeting `expected_speakers` keeps the fixed-count complete-linkage path. See [ADR 0058](adr/0058-strict-diarization-accuracy-evaluation.md) for the accuracy protocol and current validation status.

### `[transfer]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max_upload_bytes` | integer | `1073741824` | The largest upload `transfer.chunk` accumulates (1 GiB); `transfer.begin` refuses a larger size hint and a chunk that would pass it is `INVALID_PARAMS` naming the limit. |
### `[hotkeys]`

Key chords, the daemon-side backend, media pause and feedback sounds (ADR 0016, [docs/hotkeys.md](hotkeys.md)). Chords use Hyprland's notation: the modifiers (`SUPER`, `CTRL`, `ALT`, `SHIFT`), a comma, the key; `SUPER + CTRL + X` is accepted too.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `backend` | `auto` \| `portal` \| `evdev` \| `none` | `auto` | The backend that delivers presses to the daemon. Compositor bindings written by `dettivo setup` call the CLI and need none; `auto` uses the portal's GlobalShortcuts (GNOME, KDE) when it answers and otherwise nothing. |
| `hold` | chord | `"F9"` | Push to talk: press starts, release stops. |
| `toggle` | chord | `"SUPER CTRL, X"` | One press starts, the next stops. |
| `cancel` | chord | `"SUPER CTRL, Escape"` | Drops the session. |
| `reinsert` | chord | `"SUPER CTRL SHIFT, X"` | Inserts the last transcript again. |
| `pause_media` | boolean | `false` | Pause MPRIS players that are playing while capturing and resume them after. |
| `sounds` | boolean | `false` | Play a short cue on start, stop and error through `pw-play`; never during a meeting. |
| `evdev_devices` | list of paths | `[]` | Input devices for the evdev backend; empty means every keyboard under `/dev/input`. Reading them needs the `input` group. |
### `[osd]`

The recording pill outside Omarchy ([docs/osd.md](osd.md)); `dettivo-osd` reads the section at start.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | boolean | `true` | Off: `dettivo-osd` exits 0 with a notice that `dettivo doctor` shows. |
| `host` | `auto` \| `layer_shell` \| `window` | `"auto"` | `auto` takes a layer-shell overlay where the compositor offers `zwlr_layer_shell_v1` and a frameless always-on-top window otherwise; the other two force one host, and `layer_shell` on a session without one exits with a notice. |
| `position` | `top` \| `bottom` \| `top_left` \| `top_right` \| `bottom_left` \| `bottom_right` | `"top"` | The edge, and corner, the pill anchors to. On Hyprland the pill takes the opposite vertical edge for a show when the focused window reaches into its band. |
| `margin` | integer | `24` | Pixels between the pill and the anchored edges. |
| `monitor` | string | `"focused"` | `focused` follows the focused output on Hyprland and takes the primary screen elsewhere; an output name such as `DP-3` pins it, and one that is not connected falls back to the primary. |
| `hide_after_ms` | integer | `1800` | How long the Inserted and Copied states stay before the pill hides. |
| `error_hide_after_ms` | integer | `4000` | How long the Error state stays. |
| `show_level` | boolean | `true` | The bars follow the live microphone level while listening; off holds them at the floor. |
| `motion` | `full` \| `reduced` | `"full"` | `reduced` holds the bars still and cuts between states; `full` follows the desktop's reduced-motion preference. |

### `[omarchy]`

What the Omarchy plugin shows in the bar and in its panel ([docs/omarchy.md](omarchy.md)). The plugin reads the section through `dettivo config get omarchy.*` and its settings sheet writes it through `dettivo config set`, so the plugin folder carries no state and `omarchy plugin update` fast-forwards.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `glyph` | `waveform` \| `dot` | `"waveform"` | The bar glyph: the six-bar mark, or a single dot. |
| `level_meter` | boolean | `true` | The glyph's bars follow the live microphone level while recording. |
| `osd` | `panel` \| `service` \| `off` | `"panel"` | `panel`: the plugin hosts the recording pill and claims `dev.dettivo.OmarchyPanel`, so `dettivo-osd` exits with a notice. `service`: the name stays unclaimed and `dettivo-osd.service` keeps the pill. `off`: no pill on Omarchy. |
| `history_items` | integer | `3` | How many recent dictations the panel lists. |
| `open_shortcut` | chord | `"SUPER SHIFT, D"` | The label beside Open Dettivo; the binding itself is the compositor's. |

### `[polish]`

The deterministic Polish pass and the app profiles (ADR 0023, [docs/polish.md](polish.md)).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `transforms` | array of strings | `["fixGrammar", "removeFillers", "smartPunctuation"]` | The global transforms. The `code` preset never runs `smartPunctuation`, whatever this says. |
| `default_preset` | string | `"generic"` | The preset an app with no profile and no built-in mapping takes: `email`, `code`, `chat`, `notes` or `generic`. |
| `default_style` | string | `"asDictated"` | The global style: `asDictated` (each preset picks its own), `formal`, `casual` or `veryCasual`. |
| `rules` | array of tables | `[]` | The named custom rules, each `{ id, name, enabled, content }` with `content` at most 500 characters. An enabled rule joins every Enhanced rewrite. Edit them with `dettivo polish rules`; the daemon writes them back keeping your comments. |
| `apps` | table of tables | Thunderbird to `email` | Per-app profiles keyed by the Wayland app id or X11 class (`"prefix*"` matches a family), each `{ preset, style?, post_processors?, custom_preset? }`. Editors, terminals, chat and note apps have built-in defaults a profile overrides. |
| `presets` | table of tables | `{}` | Custom presets keyed by name, each `{ base, style, transforms?, custom_rules?, post_processors?, model? }`; an app profile names one in `custom_preset`. `model` is the model the Enhanced request asks the Ollama or OpenAI-compatible endpoint for under that preset; the local engine is bound to `[llm] model` and keeps it, and the policy hash names the model that answered. |

### `[llm]`

The language model provider layer behind the `enhanced` mode (ADR 0023, [docs/polish.md](polish.md)).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `provider` | string | `"auto"` | `auto` (the first available of `local`, `ollama`, `openai_compatible`), or one of them by name. `local` is the daemon's own engine, `dettivo-engine-llm`, and answers as soon as `model` is on disk. |
| `model` | string | `"qwen3-4b-instruct-2507"` | The catalogue language model the local engine loads for Enhanced: `qwen3-4b-instruct-2507`, `qwen3-1.7b` (fast), `qwen3-4b` (quality) or `qwen3-8b`. `dettivo llm download --model <id>` fetches it ([docs/models.md](models.md)). |
| `analysis_model` | string | `""` | The catalogue model meeting analysis loads; empty means `model`. |
| `polish_experiment` | string | `""` | A sideloaded polish fine-tune Enhanced runs on instead of `model` ([docs/polish-models.md](polish-models.md)): the name of a manifest under `experiments_dir` (`current` reads `current.json`, the macOS manifest shape). Empty keeps the catalogue model; `dettivo llm experiment use` and `clear` write it. A manifest that does not resolve is reported by name in `llm.models.status` and `dettivo doctor`, and Enhanced runs on `model`. |
| `experiments_dir` | string | `""` | Where sideloaded fine-tunes and their manifests live; empty means `<models_dir>/polish-experiments`. |
| `ollama_url` | string | `"http://127.0.0.1:11434"` | Where Ollama answers; detection is a `GET /api/tags` against it. A redirect is a failure, never followed. |
| `ollama_model` | string | `"qwen3:4b-instruct"` | The Ollama model. When the endpoint answers but the model is missing, `llm.providers.list` prints the `ollama pull` command. |
| `endpoint_url` | string | `""` | An OpenAI-compatible base URL; `/v1/chat/completions` is appended. A host other than localhost is used only after `dettivo llm trust <url>`. A redirect from the endpoint is a failure, never followed: the transcript reaches the confirmed host alone, so name the final URL here. |
| `endpoint_model` | string | `""` | The model that endpoint is asked for. |
| `api_key_file` | string | `""` | A file (mode 0600) holding the endpoint's API key. The Secret Service item (service `dettivo`, key `llm-api-key`) is read first. |
| `trusted_endpoints` | array of strings | `[]` | The non-loopback endpoints you confirmed, in canonical form. `dettivo llm trust` appends here; delete a line to withdraw it. |
| `timeout_ms` | integer | `8000` | The whole Enhanced pass must finish within this; then the Polish text is inserted and the notice says `provider_unavailable`. |
| `max_retries` | integer | `2` | Repair attempts after a rejected rewrite, inside the same budget. |

### `[qa]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | boolean | `false` | QA mode: the daemon accepts the QA rig's fake capture and insertion targets (ADR 0011). `DETTIVO_QA=1` turns it on without editing the file. |

### `[mcp]`

The MCP server behind `dettivo mcp serve` (ADR 0019, [docs/mcp.md](mcp.md)).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `hardened` | boolean | `false` | `dettivo mcp config` writes the `DETTIVO_IPC_TOKEN` placeholder into every host entry, as `--hardened` does, and `dettivo mcp check` reports hardened mode. Pair it with `[ipc] auth_mode = "peer_token"`. |
| `max_message_bytes` | integer | `2000000` | Longest MCP message accepted or produced over stdio, the macOS bound. A longer message is refused with the cap named (`RATE_LIMITED_LOCAL`) and the session continues. |

### `[rest]`

The loopback REST shim (ADR 0028, [docs/rest.md](rest.md)). Read at daemon start.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | boolean | `false` | The daemon hosts the listener from its next start; off, `dettivo rest serve` hosts it on demand. Every request needs the shared token (`DETTIVO_IPC_TOKEN`, the Secret Service item, or the `0600` token file); with none, no listener. |
| `port` | integer | `45831` | The TCP port; `0` takes an ephemeral one, reported in `system.capabilities.rest.port`. Applies at the next daemon start. |
| `bind` | string | `"127.0.0.1"` | A loopback address only (`127.0.0.1`, `::1`, `localhost`); anything else fails validation naming `rest.bind`, and the file is refused (a start stops, an edit keeps the values in force) with no listener on the refused address. |
| `max_body_bytes` | integer | `52428800` | The largest request body (50 MiB); a longer one is answered `413` before it is read. |
| `request_timeout_ms` | integer | `30000` | The time one request may take end to end; past it the answer is `500` naming the key. |

## Keys by route

Every key above has an editor on one of the app's settings routes ([docs/app.md](app.md#settings)), and every route names the keys it writes in its "This route writes" block. `dettivo app open settings.<section> --id <key>` opens the row that edits a key; `scripts/lint-settings-keys.sh` fails when the schema gains a key no route edits and no reason covers.

| Route | Keys |
|---|---|
| `settings.general` (Settings / General) | `daemon.log_level`, `daemon.shutdown_timeout_ms`, `audio.input_device`, `audio.level_interval_ms`, `dictation.language`, `dictation.max_duration_seconds`, `dictation.silence_peak_threshold`, `paths.data_dir`, `paths.models_dir`, `osd.enabled`, `osd.host`, `osd.position`, `osd.margin`, `osd.monitor`, `osd.hide_after_ms`, `osd.error_hide_after_ms`, `osd.show_level`, `osd.motion`, `omarchy.glyph`, `omarchy.level_meter`, `omarchy.osd`, `omarchy.open_shortcut`, `omarchy.history_items` |
| `settings.vocabulary` (Settings / Vocabulary) | `dictation.vocabulary` |
| `settings.hotkeys` (Settings / Hotkeys) | `hotkeys.hold`, `hotkeys.toggle`, `hotkeys.cancel`, `hotkeys.reinsert`, `hotkeys.backend`, `hotkeys.pause_media`, `hotkeys.sounds`, `hotkeys.evdev_devices` |
| `settings.models` (Settings / Models) | `speech.provider`, `speech.model`, `speech.meeting_model`, `speech.parakeet_model_id`, `llm.model`, `engines.stt_idle_seconds`, `engines.llm_idle_seconds`, `engines.directory`, `engines.whisper.backend`, `engines.parakeet.backend`, `engines.llm.backend`, `engines.llm.context_length`, `engines.llm.max_tokens`, `engines.diarize.backend`, `engines.diarize.threads`, `models.max_concurrent_downloads`, `models.verify_on_start`, `models.catalogue_file` |
| `settings.polish` (Settings / Polish) | `dictation.mode`, `dictation.spoken_punctuation`, `dictation.replacements`, `dictation.protect_tokens`, `polish.transforms`, `polish.default_preset`, `polish.default_style`, `llm.provider`, `llm.ollama_url`, `llm.ollama_model`, `llm.endpoint_url`, `llm.endpoint_model`, `llm.api_key_file`, `llm.trusted_endpoints`, `llm.timeout_ms`, `llm.max_retries`, `llm.polish_experiment`, `llm.experiments_dir` |
| `settings.insertion` (Settings / Insertion) | `insert.backend`, `insert.paste_keys`, `insert.terminal_app_ids`, `insert.self_app_ids`, `insert.inter_key_delay_ms`, `insert.restore_clipboard`, `insert.clipboard_restore_delay_ms`, `insert.undo_window_ms` |
| `settings.meetings` (Settings / Meetings) | `speech.meeting_model`, `llm.analysis_model`, `history.keep_audio`, `history.audio_retention_days`, `history.max_items`, `history.artifacts`, `history.db_path`, `history.max_import_seconds`, `transcribe.chunk_seconds`, `transcribe.overlap_seconds`, `transcribe.safety_margin_seconds`, `transcribe.silence_rms_floor`, `transcribe.filler_filter`, `transfer.max_upload_bytes`, `audio.system_source`, `meetings.keep_audio`, `meetings.artifacts`, `meetings.checkpoint_interval_seconds`, `meetings.live`, `meetings.live_window_ms`, `meetings.live_tick_ms`, `meetings.live_overlap_ms`, `meetings.speech_floor_rms`, `meetings.boundary_merge_gap_ms`, `meetings.cross_source_padding_ms`, `meetings.diarization.enabled`, `meetings.diarization.auto`, `meetings.diarization.model`, `meetings.diarization.pause_ms`, `meetings.diarization.nearest_turn_ms`, `meetings.diarization.min_speaker_share`, `meetings.diarization.max_speakers`, `meetings.diarization.clustering_threshold`, `meetings.delete_artifact_policy`, `meetings.analysis.auto`, `meetings.analysis.timeout_ms`, `meetings.analysis.chunk_chars`, `meetings.analysis.provider` |
| `settings.agents` (Settings / Agents) | `ipc.auth_mode`, `ipc.socket`, `ipc.token_file`, `ipc.max_line_bytes`, `mcp.hardened`, `mcp.max_message_bytes`, `rest.enabled`, `rest.bind`, `rest.port`, `rest.max_body_bytes`, `rest.request_timeout_ms` |
| `settings.diagnostics` (Settings / Diagnostics) | `qa.mode` |

Three keys have no row and are edited in the file or through the command line: `polish.rules`, an array of tables edited through `dettivo polish rules` and the Polish route's rule list; `polish.apps`, a table of tables edited through `dettivo polish apps` and the Polish route's app list; `polish.presets`, a table of tables with nested lists; edited in the file.

## Environment variables

| Variable | Effect |
|---|---|
| `DETTIVO_CONFIG` | Path of the configuration file. |
| `DETTIVO_IPC_SOCKET` | Socket path (daemon and `dettivo`). |
| `DETTIVO_IPC_TOKEN` | Shared token in `peer_token` mode (daemon and `dettivo`). |
| `DETTIVO_DATA_DIR` | Data directory. |
| `DETTIVO_QA` | `1`, `true`, `yes` or `on` turns QA mode on. |
| `RUST_LOG` | Log filter for one daemon run, over `daemon.log_level`. |
| `DETTIVO_MCP_DEBUG` | `1` makes `dettivo mcp serve` log every message on standard error. |

## Through the contract

The same operations are available to any client as `config.get`, `config.set`, `config.unset`, `config.validate`, `config.path` and `config.print_default`, listed in `system.capabilities.config.methods`. The shapes are in [docs/api/linux-deltas.md](api/linux-deltas.md) and the fixtures under `crates/dettivo-proto/fixtures/config/`.
