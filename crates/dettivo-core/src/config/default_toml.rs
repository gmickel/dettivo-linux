//! The default `config.toml`: every key present with its default value
//! and a comment. `dettivo config print-default` prints exactly this, and
//! `schema.rs` asserts it parses to `Config::default()`.

/// The default file, every key present with its default value and a
/// comment. `dettivo config print-default` prints exactly this.
pub const DEFAULT_TOML: &str = r#"# Dettivo configuration. Every key is documented in docs/config.md.
# This commented reference also ships at /usr/share/doc/dettivo/config.example.toml.
# Compare it with your file; copying over an existing config replaces its values.
# The app opens in Simple settings; Advanced reveals paths and tuning.
# Reset removes one override. Missing keys use these defaults; unrelated keys stay.
#
# This file is the source of truth: the GUI settings routes edit it, the CLI
# reads and writes every key (`dettivo config get|set|unset`), and the daemon
# reloads it when it changes. Comments and ordering survive edits made
# through the daemon. Environment overrides: DETTIVO_CONFIG (this file's
# path), DETTIVO_IPC_SOCKET, DETTIVO_DATA_DIR, DETTIVO_QA.

[daemon]
# Lowest level written to the journal: error, warn, info, debug, trace.
# No level ever logs transcript text, audio or prompts.
log_level = "info"
# How long a graceful shutdown may take before the process exits anyway.
shutdown_timeout_ms = 5000

[ipc]
# "peer": any process of the same user may connect (SO_PEERCRED).
# "peer_token": the same, plus the shared token from DETTIVO_IPC_TOKEN, the
# Secret Service item (service "dettivo", key "ipc-token") or token_file.
auth_mode = "peer"
# Socket path. Empty means $XDG_RUNTIME_DIR/dettivo/dettivo.sock; the
# systemd socket unit and DETTIVO_IPC_SOCKET override it.
socket = ""
# Token file for peer_token mode, must be mode 0600. Empty means
# $XDG_CONFIG_HOME/dettivo/ipc.token.
token_file = ""
# Longest request line the daemon accepts, in bytes. A longer line is
# answered with INVALID_PARAMS and the connection stays open.
max_line_bytes = 1048576

[paths]
# History and settings data. Empty means $XDG_DATA_HOME/dettivo.
data_dir = ""
# Model files. Empty means <data_dir>/models.
models_dir = ""

[insert]
# "auto" tries, in order, the Wayland virtual keyboard, libei through the
# RemoteDesktop portal, ydotool, xdotool, clipboard plus a paste keystroke,
# then clipboard only. A backend name pins it; an unavailable pin fails with
# its reason instead of falling back.
backend = "auto"
# Paste keystroke per app id for the clipboard path (`ctrl+v`,
# `ctrl+shift+v`, `shift+insert`). Terminals default to ctrl+shift+v,
# everything else to ctrl+v.
paste_keys = {}
# App ids that are terminals and take ctrl+shift+v.
terminal_app_ids = ["foot", "footclient", "com.mitchellh.ghostty", "ghostty", "Alacritty", "alacritty", "kitty", "org.wezfurlong.wezterm", "wezterm", "xterm", "org.gnome.Console", "org.gnome.Terminal", "org.kde.konsole"]
# App ids of Dettivo's own windows. Insertion into one of them fails with
# target_is_self and never pastes.
self_app_ids = ["dettivo", "dettivo-app", "dettivo-osd", "dettivo-sheet"]
# Pause between typed keys, in milliseconds.
inter_key_delay_ms = 2
# Put the previous clipboard back after a clipboard insertion.
restore_clipboard = true
# How long the restore waits for the target to take the paste, in
# milliseconds. A clipboard the user changed meanwhile is left alone.
clipboard_restore_delay_ms = 300
# How long after an insertion `dettivo insert undo` may take it back.
undo_window_ms = 5000

[qa]
# QA mode: accept the QA rig's fake capture and insertion targets.
# DETTIVO_QA=1 turns it on without editing this file.
mode = false

[engines]
# Directory searched for engine binaries (dettivo-engine-whisper, ...)
# before PATH. Empty means the daemon's own directory and PATH.
directory = ""
# Seconds a speech engine stays warm after its last request before it is
# unloaded and its process ends, returning GPU memory.
stt_idle_seconds = 300
# The same for the language model engine (dettivo-engine-llm).
llm_idle_seconds = 600

[engines.whisper]
# Compute backend for the Whisper engine: auto (Vulkan when a device is
# installed and the model loads there, else CPU), vulkan (Vulkan or a
# failed load), cpu. DETTIVO_FORCE_CPU=1 overrides both engines.
backend = "auto"

[engines.parakeet]
# The same for the Parakeet engine.
backend = "auto"

[engines.llm]
# The same for the language model engine; auto falls back to the CPU with
# the reason when the model does not fit the device's free memory.
backend = "auto"
# The context length in tokens the model is loaded with.
context_length = 4096
# The most tokens one Enhanced rewrite may generate.
max_tokens = 1024

[engines.diarize]
# Compute backend: auto (CUDA when its provider and runtime load, else CPU
# with the reason), cpu, cuda (CUDA or a failed load). DETTIVO_FORCE_CPU=1
# overrides this choice. The CUDA drop-in needs CUDA 13 and cuDNN 9.
backend = "auto"
# Threads the diarization engine runs its models on; 0 is the core count
# capped at four.
threads = 0

[speech]
# Speech provider: whisper or parakeet.
provider = "whisper"
# Model id under <models>/<provider>/; a session freezes it at start.
model = "large-v3-turbo"
# The meeting model; empty means the dictation model.
meeting_model = ""
# The Parakeet model `dettivo speech selection set --provider parakeet`
# switches to when `model` is not a Parakeet model.
parakeet_model_id = "parakeet-v3"

[models]
# Downloads that may run at once.
max_concurrent_downloads = 1
# Re-hash models on disk that have no verified manifest when the daemon
# starts; a mismatch quarantines the file.
verify_on_start = true
# A catalogue file instead of the built-in one (mirrors, tests). Empty
# means the built-in catalogue.
catalogue_file = ""

[audio]
# PipeWire node name to capture from (see `dettivo doctor` or audio.devices).
# Empty follows the default source, including when the default changes.
input_device = ""
# How often a level sample (RMS and peak) is published, in milliseconds.
level_interval_ms = 50
# A meeting's system track: "default_monitor" captures the default sink's
# monitor and follows the default when it changes; a sink's node name pins
# that sink's monitor.
system_source = "default_monitor"

[dictation]
# Language code (en, de, ...) or auto.
language = "auto"
# raw (the engine's text with the raw pipeline), deterministic_polish (the
# Polish pass, no model; "polish" is the contract's alias) or enhanced (Polish,
# then a language model rewrite through [llm]; falls back to Polish with a
# notice when no provider answers in time).
mode = "raw"
# Words the engine is primed with: names, products, jargon.
vocabulary = []
# Spoken punctuation ("comma", "period", "new line") becomes marks.
spoken_punctuation = true
# File names, paths, URLs, email addresses, versions and dotted identifiers
# (index.ts, src/app, example.com, v1.2.3, foo.bar) pass through untouched:
# no sentence end on a dot inside one, no replacement inside one.
protect_tokens = true
# The longest take in seconds before the session stops itself.
max_duration_seconds = 300
# A take whose peak level stays under this is treated as silence.
silence_peak_threshold = 0.01
# Whole-word replacements applied after recognition, case-insensitive:
# replacements = { teh = "the", dettivo = "Dettivo" }
replacements = {}

[history]
# Keep the take of every finished dictation beside its item, under
# <data_dir>/dictations/<id>/, so it can be re-run with another model.
keep_audio = true
# Days after which retained audio is removed by the hourly sweep; 0 keeps it.
audio_retention_days = 30
# Items kept; the oldest beyond this are removed by the sweep. 0 keeps all.
max_items = 0
# What lives beside the database and what a delete removes: "keep" (audio
# and metadata.json), "audio_only" (no sidecar), or "none" (rows only).
artifacts = "keep"
# The SQLite database. Empty means <data_dir>/dettivo.db.
db_path = ""
# The longest audio file an import accepts, in seconds; a longer one is
# refused after the probe, before anything is decoded.
max_import_seconds = 14400

[transcribe]
# The chunked pipeline behind imports and re-runs (docs/history.md). Audio
# is cut into windows of this many seconds. Quiet edges are trimmed before
# recognition so automatic language detection starts near speech.
chunk_seconds = 30
# Seconds two consecutive chunks share; the merger reconciles them by
# timestamp.
overlap_seconds = 2
# Seconds before a chunk's nominal end inside which the cut moves to the
# quietest point, so a chunk rarely ends mid-word.
safety_margin_seconds = 5
# RMS (0 to 1) under which a chunk is silence: it never reaches the engine
# and adds no text.
silence_rms_floor = 0.0065
# Drop the known filler hallucinations ("thanks for watching", ...) before
# the text is stored.
filler_filter = true

[meetings]
# Keep microphone.wav (one per take) and system.wav under
# <data_dir>/meetings/<id>/ after the meeting; off removes them at stop.
keep_audio = true
# Seconds between two writes of live-checkpoint.json while recording; a
# daemon that dies mid-meeting recovers the meeting from the last one.
checkpoint_interval_seconds = 15
# What lives in the meeting directory once the meeting stopped: "keep"
# (audio, journal.jsonl, the take sidecars and metadata.json), "audio_only"
# (no metadata.json), or "none" (the directory is removed).
artifacts = "keep"
# Transcribe while the meeting records: windows of both tracks go through
# the engine and arrive as meeting.segment events (docs/meetings.md). Off
# runs the finalisation alone when the meeting stops.
live = true
# The longest live window sent to the engine, in milliseconds; a window
# grows towards it while the engine catches up.
live_window_ms = 3000
# Milliseconds two consecutive live windows share, so a word cut by one
# window is whole in the next.
live_overlap_ms = 450
# Milliseconds of new audio that cut the next live window.
live_tick_ms = 900
# A live segment this far behind the newest audio is final; nearer ones
# stay provisional and the next window may replace them.
boundary_merge_gap_ms = 1200
# A microphone segment that is a filler and sits within this many
# milliseconds of a long remote segment is the speakers heard through the
# microphone, and is dropped.
cross_source_padding_ms = 800
# RMS (0 to 1) a live window must reach to be sent to the engine; 70 % of
# it keeps a window going while the previous one was speech.
speech_floor_rms = 0.0065
# The artifact policy `dettivo meetings delete` and the app use when none
# is named: transcript_only (the texts, segments and analysis go, the
# notes and the audio stay), transcript_and_audio (the takes go too, the
# facts and the notes stay) or all (the row and the directory go).
delete_artifact_policy = "all"

[meetings.analysis]
# Analyse every meeting once it finalises (a summary, the decisions, the
# action items) through the language model provider; off leaves it to
# `dettivo meetings analyze <id>`.
auto = true
# The whole analysis must finish within this; then it is failed with
# provider_unavailable and can be run again.
timeout_ms = 60000
# Characters of transcript per model call; a longer transcript is analysed
# in parts and the parts are merged in one final call.
chunk_chars = 12000
# The provider the analysis asks: local, ollama, openai_compatible or auto;
# empty follows [llm] provider. [llm] analysis_model names the local model.
provider = ""

[meetings.diarization]
# Learn who spoke once a meeting is finalised (docs/meetings.md): the
# system track (the whole microphone track of a room-audio meeting) goes
# through dettivo-engine-diarize and every remote segment gets a speaker.
enabled = true
# Run the pass by itself when the finalisation completes; off leaves it to
# `dettivo meetings diarize <id>`.
auto = true
# The catalogue model set under <models>/diarize/
# (`dettivo speech download --provider diarize --model diarization-en`).
model = "diarization-en"
# A segment is labelled only when at least this share of its span lies
# inside diarized speech...
min_coverage = 0.25
# ...and the winning speaker holds at least this share of that speech;
# otherwise the segment stays unlabelled.
min_speaker_share = 0.6
# The speaker count the clustering is told; 0 lets it decide.
max_speakers = 0
# Initial average-linkage cosine cutoff, followed by low-support cluster
# reassignment. The final speaker count need not vary monotonically.
clustering_threshold = 0.6

[transfer]
# The largest upload transfer.chunk accumulates, in bytes (1 GiB).
max_upload_bytes = 1073741824

[hotkeys]
# The daemon-side hotkey backend: auto, portal, evdev or none. Compositor
# bindings written by `dettivo setup` call the CLI and need no backend;
# auto uses the portal's GlobalShortcuts (GNOME, KDE) when it answers.
backend = "auto"
# Chords in Hyprland notation: modifiers (SUPER, CTRL, ALT, SHIFT), a comma,
# the key. Omarchy's Voxtype keys are the defaults.
# Hold to talk: press starts, release stops.
hold = "F9"
# One press starts, the next stops.
toggle = "SUPER CTRL, X"
# Drop the session.
cancel = "SUPER CTRL, Escape"
# Insert the last transcript again.
reinsert = "SUPER CTRL SHIFT, X"
# Pause playing MPRIS players while capturing and resume them after.
pause_media = false
# Play a short sound on start, stop and error (never during a meeting).
sounds = false
# Input devices for the evdev backend (/dev/input/event*); empty means every
# keyboard. Reading them needs the `input` group.
evdev_devices = []

[osd]
# The recording pill outside Omarchy (dettivo-osd; the Omarchy plugin hosts
# the same pill as a panel). Off: dettivo-osd exits with a notice.
enabled = true
# auto: a layer-shell overlay where the compositor offers one, else a
# frameless always-on-top window. layer_shell or window force one host.
host = "auto"
# top, bottom, top_left, top_right, bottom_left or bottom_right.
position = "top"
# Distance from the anchored edges, in pixels.
margin = 24
# focused: the focused output on Hyprland, the primary elsewhere. Or an
# output name such as DP-3; an output that disappears falls back to primary.
monitor = "focused"
# How long the inserted and copied states stay before the pill hides.
hide_after_ms = 1800
# How long the error state stays before the pill hides.
error_hide_after_ms = 4000
# The bars follow the live microphone level while listening.
show_level = true
# full follows the desktop's reduced-motion setting; reduced holds the bars
# still and cuts between states.
motion = "full"

[omarchy]
# The bar widget and the panel the Omarchy plugin draws (docs/omarchy.md).
# The plugin reads this section through `dettivo config get` and writes it
# through `dettivo config set`; its folder carries no state of its own.
# waveform: the six-bar mark. dot: a single dot.
glyph = "waveform"
# The glyph's bars follow the live level while recording.
level_meter = true
# panel: the plugin hosts the recording pill and dettivo-osd steps aside.
# service: dettivo-osd keeps the pill. off: no pill on Omarchy.
osd = "panel"
# How many recent dictations the panel lists.
history_items = 3
# The shortcut label beside Open Dettivo, in Hyprland's chord notation.
open_shortcut = "SUPER SHIFT, D"

[mcp]
# `dettivo mcp config` adds the DETTIVO_IPC_TOKEN placeholder to every host
# entry, as `--hardened` does; pair it with [ipc] auth_mode = "peer_token".
hardened = false
# Longest MCP message accepted or produced over stdio, in bytes (the macOS
# bound). A longer message is refused naming the cap; the session continues.
max_message_bytes = 2000000

[rest]
# The daemon hosts the loopback REST shim at start (docs/rest.md); off, and
# `dettivo rest serve` hosts it on demand. Every request needs the shared
# token (DETTIVO_IPC_TOKEN, the Secret Service item, or the 0600 token file).
enabled = false
# The TCP port; 0 takes an ephemeral port, reported in system.capabilities.
port = 45831
# A loopback address only (127.0.0.1 or ::1); anything else fails validation.
bind = "127.0.0.1"
# The largest request body, in bytes (50 MiB); a longer one is answered 413.
max_body_bytes = 52428800
# The time one request may take end to end, in milliseconds.
request_timeout_ms = 30000

[polish]
# The deterministic Polish pass (docs/polish.md). Global transforms; the
# code preset never runs smartPunctuation.
transforms = ["fixGrammar", "removeFillers", "smartPunctuation"]
# The preset an app without a profile takes: email, code, chat, notes, generic.
default_preset = "generic"
# The global style: asDictated (each preset picks its own), formal, casual,
# veryCasual.
default_style = "asDictated"
# Named custom rules, edited through `dettivo polish rules`; an enabled rule
# joins every Enhanced rewrite. Each is { id, name, enabled, content } with
# content up to 500 characters.
rules = []
# Per-app presets keyed by the Wayland app id or X11 class ("prefix*" matches
# a family); style, post_processors and custom_preset may override the
# preset's defaults. Thunderbird is the shipped example; code editors,
# terminals and chat apps have built-in defaults a profile overrides.
apps = { "org.mozilla.Thunderbird" = { preset = "email" } }
# Custom presets keyed by name: { base, style, transforms, custom_rules,
# post_processors, model }; an app profile names one in custom_preset.
presets = {}

[llm]
# The provider behind Enhanced: auto (the first available of local, ollama,
# openai_compatible), local (dettivo-engine-llm with a catalogue model),
# ollama or openai_compatible.
provider = "auto"
# The catalogue model the local engine loads for Enhanced:
# qwen3-4b-instruct-2507 (default), qwen3-1.7b (fast), qwen3-4b (quality),
# qwen3-8b. `dettivo llm download --model <id>` fetches it.
model = "qwen3-4b-instruct-2507"
# The model meeting analysis loads; empty means `model`.
analysis_model = ""
# A sideloaded polish fine-tune Enhanced runs on instead of `model`: the name
# of a manifest under experiments_dir ("current" reads current.json, the
# macOS manifest shape). Empty keeps the catalogue model; `dettivo llm
# experiment use` sets it and `dettivo llm experiment status` shows it.
polish_experiment = ""
# Where sideloaded fine-tunes and their manifests live. Empty means
# <models_dir>/polish-experiments.
experiments_dir = ""
# Where Ollama answers; auto detects it through GET /api/tags.
ollama_url = "http://127.0.0.1:11434"
# The Ollama model (`ollama pull qwen3:4b-instruct`).
ollama_model = "qwen3:4b-instruct"
# An OpenAI-compatible base URL; /v1/chat/completions is appended. A host
# other than localhost is used only after `dettivo llm trust <url>`.
endpoint_url = ""
# The model the endpoint is asked for.
endpoint_model = ""
# A file (mode 0600) holding the endpoint's API key; the Secret Service item
# (service "dettivo", key "llm-api-key") is read first.
api_key_file = ""
# Remote endpoints you confirmed; `dettivo llm trust` appends here.
trusted_endpoints = []
# The whole Enhanced pass must finish within this; then the Polish text is
# inserted and the notice says provider_unavailable.
timeout_ms = 8000
# Repair attempts after a rejected rewrite, within the same budget.
max_retries = 2
"#;
