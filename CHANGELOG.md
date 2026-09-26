# Changelog

Every release of Dettivo for Linux is listed here in the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) shape. The release workflow takes the section for the tag's version as the release notes, so a tag without its section fails before anything is built ([docs/RELEASING.md](docs/RELEASING.md)).

## [Unreleased]

### Fixed

- The install commands download the release package and its checksum and install the local file. pacman refused the direct URL because it asks for a signature the releases do not publish. Reported by @gmickel.

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
