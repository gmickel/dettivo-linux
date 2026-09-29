# Changelog

Every release of Dettivo for Linux is listed here in the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) shape. The release workflow takes the section for the tag's version as the release notes, so a tag without its section fails before anything is built ([docs/RELEASING.md](docs/RELEASING.md)).

## [Unreleased]

### Fixed

- Dictation works while a game holds the GPU. A Parakeet engine that started with too little GPU memory left failed every dictation with `vk::Device::allocateMemory: ErrorOutOfDeviceMemory` until it was restarted; it now moves to the CPU and finishes the dictation, and any engine that reports the GPU out of memory is restarted on the CPU instead of kept warm ([ADR 0080](docs/adr/0080-whisper-cpp-carries-a-dtw-median-filter-guard.md)). Reported by @gmickel.
- Re-run on a meeting whose transcription failed, was cancelled, stopped or cut short transcribes it again from its retained audio, the same as `dettivo meetings recover <id>`, and the meeting view follows the new run. It was always disabled. When the audio is gone, or the meeting is still recording or being transcribed, Re-run stays disabled and its tooltip says why. Reported by @gmickel.
- The progress bar under a running meeting stage stays inside the processing strip. While it swept, it drew a line from the window's left edge across the sidebar. Reported by @gmickel.

## [0.4.1] - 2026-09-29

### Fixed

- Meetings, imports and re-runs transcribed with Whisper no longer fail on a stretch of audio that crashed the engine. 0.4.0's word timing aborted whisper.cpp when the last part of a chunk lasted about a tenth of a second, which failed a whole meeting at one chunk; the bundled whisper.cpp now skips word timing for such a sliver instead ([ADR 0080](docs/adr/0080-whisper-cpp-carries-a-dtw-median-filter-guard.md)). A meeting that failed this way transcribes when you re-run it.
- A chunk the engine crashes on twice becomes the line `[Not transcribed: speech recognition failed on this part]` and the rest of the transcript is kept, up to three such lines per transcript.
- On meetings longer than a few minutes the speaker pass runs its voiceprint and voice check again. The voice embeddings of the whole meeting went back in one reply too large for the engine to send, and the pass carried on without them ([ADR 0080](docs/adr/0080-whisper-cpp-carries-a-dtw-median-filter-guard.md)).
- Meeting analysis finishes when a game holds the GPU. The language model falls back to the CPU when the GPU has no room for it, a model on the CPU keeps its working memory off the GPU, and `[meetings.analysis] timeout_ms` defaults to 30 minutes, which covers about two hours of meeting on the CPU; on the GPU an hour's analysis still takes about a minute.
- An engine that crashes on the GPU, for instance when a game holds all of the GPU's memory, carries on on the CPU instead of crashing again, and every engine crash logs the engine's last error output.
- Settings saves vocabulary terms and polish transforms again. Adding a term in Settings / Vocabulary, or switching a transform in Settings / Polish, sent no list at all, and the page answered `dictation.vocabulary: expected a list of strings, got null`. Reported by @gmickel.
- `dettivo config set` takes a list or table typed as it appears in the file, such as `dettivo config set dictation.vocabulary '["SapienXT", "Dettivo"]'`. It used to store the brackets inside one entry; the comma form (`SapienXT, Dettivo`) still works. Reported by @gmickel.

## [0.4.0] - 2026-09-28

### Added

- The speaker pass can run NVIDIA Nemotron 3 Diarization: download `diarize/nemotron-3-diarization` and set `[meetings.diarization] model = "nemotron-3-diarization"`. It runs through the new `dettivo-engine-nemotron` on the GPU with Vulkan (or CUDA with the drop-in) and on the CPU otherwise, and diarizes an hour of meeting in about 15 seconds on an RTX 4090 through Vulkan. A meeting that expects more than eight speakers, or a machine without the model, runs the current engine instead and says why in the meeting's `diarization.fallback_reason`. The current engine stays the default: on the final bench the two tie on held-out AMI and German meetings, and the current engine labels English calls with several remote voices better ([ADR 0073](docs/adr/0073-nemotron-3-diarization-runs-through-nemo-speech-cpp.md), [ADR 0078](docs/adr/0078-the-final-bench-keeps-the-sherpa-onnx-set-the-default.md)).
- `[engines.diarize] backend` accepts `vulkan`, and Settings / Models offers it.

### Changed

- Meetings give almost every remote line a speaker. Each sentence takes the speaker who holds most of it, and a line with no diarized speech takes the nearest speaker within 10 seconds. On the final bench about 1 remote line in 300 is left without a speaker, where it was about 1 in 5. A line whose sentences belong to two speakers becomes two lines. `[meetings.diarization]` gains `pause_ms` and `nearest_turn_ms`. `min_coverage` is retired: a file that still sets it keeps loading, and the value is ignored. `min_speaker_share` now applies per sentence and defaults to 0.

- Meetings, imports and re-runs transcribed with Whisper store every word with its start, end and confidence, and each line's start and end follow its words instead of Whisper's segment timestamps, which drift by seconds in long recordings. The speaker pass gets 1.4 points fewer AMI words wrong with the current diarization engine and 2.2 with Nemotron. Named lines with the wrong speaker stay where they were. `[meetings.diarization] pause_ms` defaults to 500.

- Meetings recorded with a system track use both tracks to name speakers. The speaker pass drops a microphone line you barely voiced, which is the other side heard through your microphone or words Whisper heard in silence. On the retained meetings that cut the time "You" lines spent on remote-only speech from 38% to 22%. A call with one remote voice gets that voice on every remote line. A silent system track means several people shared your microphone, so it is diarized like room audio. The pass also learns your voice from your microphone and names a remote line in your voice You. The voiceprint stays on this machine in `<data_dir>/voiceprint.json`. `[meetings.diarization]` gains `bleed_min_voiced`, `single_remote`, `shared_mic`, `voiceprint` (set it to `false` to opt out) and `voice_match`.

- The speaker pass checks each sentence against the voices of the speakers it found, and moves a sentence that clearly sounds like another speaker to that speaker. On AMI meetings that cuts the words under the wrong speaker from 21.1% to 18.6% with the current engine and from 12.5% to 12.0% with Nemotron, for about 25 seconds per meeting hour on the CPU. `[meetings.diarization] voice_check = false` turns it off ([ADR 0076](docs/adr/0076-each-sentence-is-checked-against-the-speakers-voices.md)).

### Fixed

- Piping the CLI into a command that stops reading early, such as `dettivo meetings segments <id> | head`, ends it quietly instead of with a Rust panic.
- A recording's take list (`takes.json`) is written through a temporary file and renamed into place, so a crash during the write can no longer leave recovery an empty list.
- A packaged daemon reports its commit as `build` in `system.version` and `dettivo doctor` instead of `dev`. Reported by @gmickel.
- The install commands download the release package and its checksum and install the local file. pacman refused the direct URL because it asks for a signature the releases do not publish. Reported by @gmickel.
- Dettivo opens while a game or another program holds all of the GPU's memory. The app, the recording pill and every other Dettivo window used to abort with no message when the GPU could not draw them; each now logs one warning and restarts once on Qt's software renderer. `[app] renderer = "software"` draws on the CPU from the start, and `dettivo app status` and `dettivo doctor` report the renderer and why ([ADR 0077](docs/adr/0077-a-window-the-gpu-cannot-draw-restarts-once-on-the-software-renderer.md)). Reported by @gmickel.

## [0.3.0] - 2026-09-26

Agents can follow a running meeting from its first word, even when they start late.

### Added

- Agents can read a running meeting's transcript so far through `meetings.segments`, so a live copilot that attaches late still sees the whole meeting. The same answer is available from `dettivo meetings segments <id>`, the MCP tool `get_meeting_segments` and `/v1/meetings/segments`. A cursor returns only new lines on the next call and keeps working through Stop until the stored transcript is complete. Every line carries its side (`you` or `remote`), and the provisional tail comes back flagged. `docs/guides/agents.md` has a live meeting copilot recipe.

### Changed

- `dettivo meetings segments <id>` prints the transcript so far during a recording instead of "no segments yet". Each line shows its side, and a `~` column marks provisional lines. `--json` prints the whole answer (segments, provisional tail, cursor) instead of the bare segment list. `--follow` started mid-meeting prints the backlog first, then streams without a gap or a duplicate.

## [0.2.0] - 2026-09-25

The first public release of Dettivo for Linux, now open source under GPL-3.0-or-later.

### Added

- Every release carries the `dettivo-bin` pacman package and its checksum, so Arch and Omarchy users can `pacman -U` it from GitHub while the AUR packages wait for AUR account registration to reopen.
- Parakeet Ultra, Moondream's retrained Parakeet TDT 0.6B v3, is available to download beside v2 and v3: the same size and languages, with lower word error on Moondream's benchmarks.
- Home and the meetings list offer Return to meeting while a recording or its final transcription is running.
- Every release publishes the Omarchy plugin to `gmickel/omarchy-dettivo` for `omarchy plugin add`, and will publish `dettivo-bin` and `dettivo` to the AUR once AUR publishing is switched on.
- Meeting titles can be renamed by clicking the title or pressing `t`; Enter saves and Escape cancels.

### Changed

- Dettivo for Linux is now licensed under GPL-3.0-or-later instead of MIT.
- The speech engines compile for one x86-64 baseline (AVX2, FMA, F16C), so the release package runs on any x86-64 CPU from 2013 on rather than only on CPUs like the build machine's.

### Fixed

- Meetings continue on the CPU when an automatic Whisper model load crashes the GPU engine.
- Recognition windows default to 30 seconds with quiet edges trimmed, so automatic language detection follows a language change inside a meeting or import.
- Quiet speech moves the recording bars; both pills use the same logarithmic level curve.
- Automatic meeting titles use the local time zone at the meeting's start.
- Whisper meeting models can be selected while Parakeet remains the dictation provider. Reported by @gmickel.
- Live meetings retain every provisional fragment, acknowledge Stop immediately, and show pending, running, completed and failed processing stages. Unassigned speaker segments remain visible. Reported by @gmickel.
- The Omarchy meeting timer advances through silence, restores its start time on reconnect, and ignores completion events from an older meeting. Reported by @gmickel.
- Long-meeting analysis splits inputs and summary groups that exceed model limits, preserves extracted decisions and actions, and respects cancellation. Reported by @gmickel.
- Omarchy shortcut setup loads the Lua bindings and checks activation before onboarding advances. Missing includes and setup failures can be retried. Reported by @gmickel.
- The Omarchy plugin ships its shared-state registration, loads its panel style without a session environment override, and keeps the idle waveform visible. Reported by @gmickel.
- The bar panel uses an inset, equal-width mode selector and flat themed action buttons.

## [0.1.0] - 2026-09-14

The first release: the native Omarchy port of the Dettivo speech workstation.

### Added

- The Rust daemon behind a socket-activated systemd user unit, the macOS IPC v1 contract with the Linux deltas, and the `dettivo` command line, MCP server and QA runner.
- Dictation into any window through the Wayland virtual keyboard with portal, uinput and clipboard fallbacks; compositor hotkeys on Hyprland, sway and niri.
- Whisper, Parakeet and a local language model as supervised ggml engine processes with the Vulkan backend and a CPU fallback; the shared model catalogue.
- The Qt Quick app, the recording pill and the Omarchy bar plugin on one QML module that resolves every value from the Omarchy theme.
- Meetings, imports, the SQLite history with full-text search, the Polish text layers and the Enhanced pass.
- The `dettivo-bin` and `dettivo` AUR recipes, the release workflow and the clean-machine install test.

### Fixed

- Release packages include the current shared controls, Omarchy meeting helpers and light icon; removed meeting components no longer remain in the package manifest.
- Release checks record a confirmed disabled primary CI workflow as an external prerequisite while preserving failed runs and unexplained missing evidence.

- Home starts meetings through disclosure and displays dictation failures. Model, configuration, REST and configured-host summaries follow their live state.
- Hidden actions and locked settings expose the correct accessibility state. Narrow settings and shorter first-run windows keep controls reachable.
- Dialogs use the shared style without replacing its drawing delegates. History waveform previews defer media initialization until playback and preserve a seek made before the first play.
- Desktop QA isolates host configuration paths, preserves text selection and Return on the private desktop, and checks complete accessibility snapshots.
- Native QML modules install under the private library directory. Package selection excludes debug archives, and failed namcap or incomplete install checks block packaging.
- Reviewed visual baselines and native keyboard verification cover all eighteen surfaces; shortcut handling preserves the hint sheet and the intended modifiers.
