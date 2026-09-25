# Architecture decision records

Each record explains one decision in the order it was made, what it gives the people who use and build Dettivo for Linux, and what it costs. Records are the main documentation of this repository. When a decision changes, a new record supersedes the old one and the old one gets a pointer forward; nothing is edited into silence.

Records from the v1 build cite "the masterplan" and requirement IDs such as FR-U1 or NFR-4. The masterplan is the product plan kept outside this repository, and each record restates in its own words the requirement it relies on.

| Record | Decision | Status |
|---|---|---|
| [0001](0001-rust-daemon-qt-quick-ui.md) | Rust behind the socket, Qt Quick for everything you see, a C++20 host for the Qt binaries | Accepted 2026-09-03 |
| [0002](0002-daemon-owns-the-contract.md) | The daemon owns the contract; every interface is a client; systemd activates the socket | Accepted 2026-09-03 |
| [0003](0003-engine-processes.md) | Inference runs in supervised engine processes | Accepted 2026-09-03 |
| [0004](0004-ggml-family-vulkan.md) | One ggml family with Vulkan by default; Sherpa-ONNX only for diarization | Accepted 2026-09-03; Parakeet for meetings settled by 0018 (dictation-only) |
| [0005](0005-local-llm-and-shared-models.md) | A local language model in v1, with the same models and fine-tunes as macOS | Accepted 2026-09-03 |
| [0006](0006-pipewire-capture.md) | PipeWire captures microphone and system audio and resamples on the way in | Accepted 2026-09-03 |
| [0007](0007-in-process-text-insertion.md) | Text insertion through an in-process Wayland virtual keyboard with portal, uinput and clipboard fallbacks | Accepted 2026-09-03 |
| [0008](0008-contract-parity.md) | The macOS IPC v1 contract is normative; deltas live in one file; fixtures prove conformance | Accepted 2026-09-03 |
| [0009](0009-config-files-first.md) | `config.toml` is the source of truth; first run is three screens that only appear when something is missing | Accepted 2026-09-03 |
| [0010](0010-studio-design-system.md) | The Studio design system resolves every visual value from the Omarchy theme; one QML module serves app, OSD and bar plugin | Accepted 2026-09-03 |
| [0011](0011-qa-drives-and-audio-rig.md) | Desktop drives through cua-driver under X11 with an in-repo fallback, a virtual PipeWire audio rig and contract fixtures | Accepted 2026-09-03 |
| [0012](0012-fast-build-then-cleanup.md) | Specs are captured without plans or reviews during the build; a cleanup phase turns the gates on | Accepted 2026-09-03 |
| [0013](0013-qt-floor-and-packaging.md) | Qt 6.8 floor; AUR packages with engine binaries as separate files | Accepted 2026-09-03 |
| [0014](0014-sqlite-history-store.md) | One SQLite file with FTS5 is the history store; artifacts sit beside it under a policy | Accepted 2026-09-04 |
| [0015](0015-osd-pill-hosting.md) | One pill component, hosted on a layer-shell overlay with a window fallback; the completion event carries the outcome; frame pacing is measured, not claimed | Accepted 2026-09-04 |
| [0016](0016-hotkeys-compositor-bindings-portal-evdev.md) | Compositor bindings drive the CLI; the portal and evdev backends live in the daemon; every start captures the focused window | Accepted 2026-09-04 |
| [0017](0017-qa-packs-and-the-dictation-report.md) | Packs compose scenarios into one release-shaped report; the completion event carries the timing split | Accepted 2026-09-04 |
| [0018](0018-parakeet-engine-dictation-only.md) | Parakeet runs on parakeet.cpp through an in-repo binding; its timestamps keep it dictation-only for now | Accepted 2026-09-04 |
| [0019](0019-mcp-hand-rolled-protocol-and-tool-mapping.md) | The MCP server is a hand-rolled JSON-RPC layer over stdio that maps the macOS tools one to one onto daemon methods | Accepted 2026-09-04 |
| [0020](0020-app-shell-thin-host-single-instance.md) | The app is a thin host around the shared module, one window per session, proven by name | Accepted 2026-09-04 |
| [0021](0021-visual-regression-gate-and-frame-pacing.md) | Every surface is held to an approved baseline on every theme and scale by a blocking job with a canary; frame pacing is judged in the drives while a surface animates | Accepted 2026-09-04 |
| [0022](0022-chunked-import-symphonia-decoder-and-overlap-merger.md) | Imports decode in-process through Symphonia and every long transcription runs through one chunked pipeline with a timestamp merger | Accepted 2026-09-04 |
| [0023](0023-polish-layers-and-the-llm-provider-layer.md) | The Polish layers are ported verbatim from macOS with their golden cases; one provider interface serves Enhanced, and a remote endpoint needs one explicit confirmation | Accepted 2026-09-04 |
| [0024](0024-first-run-three-screens-config-writes.md) | First run is three screens of config writes, shown only when a key or a model is missing, with one allowance to dictate into itself | Accepted 2026-09-04 |
| [0025](0025-history-detail-facts-playback-and-export-file.md) | The history detail reads one answer, plays the take in-process and writes an export where the portal says | Accepted 2026-09-04 |
| [0026](0026-local-llm-engine-llama-cpp-and-the-gguf-catalogue.md) | The local language model runs llama.cpp through the engine protocol with the macOS catalogue in GGUF, and the `local` provider is the default once a model is on disk | Accepted 2026-09-04 |
| [0027](0027-two-stream-meeting-capture-journal-and-checkpoint.md) | A meeting is two takes on one clock, a journal and a live checkpoint; the daemon's next start promotes what a killed one left | Accepted 2026-09-04 |
| [0028](0028-rest-shim-hand-rolled-server-and-status-map.md) | The REST shim is a hand-rolled HTTP/1.1 server over the router, loopback only, token on every request, with the macOS status map | Accepted 2026-09-04 |
| [0029](0029-nfr-calibration-and-the-benchmark-suite.md) | The NFR targets are calibrated per hardware tier from a checked-in benchmark report, and the daemon names the tier | Accepted 2026-09-05 |
| [0030](0030-omarchy-plugin-in-repo-folder-mirror-and-panel-hosted-pill.md) | The Omarchy plugin is a folder in this repository, mirrored for `omarchy plugin add`, and its panel hosts the pill | Accepted 2026-09-05, amended by 0051 |
| [0031](0031-live-windowed-meeting-transcription-and-cross-source-suppression.md) | A meeting is transcribed twice by one merger: live in windows while it records, and in full from every take when it stops | Accepted 2026-09-05 |
| [0032](0032-polish-fine-tune-sideload-and-the-eval-harness.md) | A private polish fine-tune sideloads from the macOS manifest, every candidate is scored by the macOS harness through the Linux engine, and promotion stays a config change | Accepted 2026-09-05 |
| [0033](0033-settings-routes-config-editor-key-registry.md) | Settings offers everyday controls and advanced access to one configuration file | Accepted 2026-09-05 |
| [0034](0034-install-layout-cuda-drop-in-and-release-workflow.md) | One install layout under `/usr` with the engines in their own directory and a reserved CUDA drop-in, two AUR recipes over one release tarball, and a tag that publishes itself after a clean-machine install test | Accepted 2026-09-05; CI cadence superseded by 0040; amended by 0051 and 0053 |
| [0035](0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md) | Diarization runs sherpa-onnx in its own engine process after the meeting, and a segment gets a speaker only under the coverage and share rule | Accepted 2026-09-05 |
| [0036](0036-meeting-notes-analysis-search-export-and-the-delete-policy.md) | A meeting keeps the user's notes and the model's analysis apart, polishes its segments at finalisation, and deletes by the contract's policy | Accepted 2026-09-05 |
| [0037](0037-gui-packs-per-surface-and-the-tree-checks.md) | The GUI is proven by one pack of four surfaces, and every captured tree is scanned for developer text and unnamed controls | Accepted 2026-09-05 |
| [0038](0038-meetings-gui-live-events-pause-gap-and-inline-dialogs.md) | The meetings screens are clients of the meeting events: the live model folds the segment stream, Pause waits for a verb, and the small dialogs are designed inline | Accepted 2026-09-05 |
| [0039](0039-meetings-pack-finalisation-path-throughput-and-the-gpu-proof.md) | The meetings pack measures throughput through the daemon's own finalisation path and proves the GPU tier did its work on the GPU | Accepted 2026-09-05, amended by 0051 |
| [0040](0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md) | Fast PR gates and one tested release package | Accepted 2026-09-05, amended by 0051 |
| [0041](0041-guides-docs-build-evidence-map-and-the-release-gate-script.md) | Four guides lead the reader, the docs build holds the pages together, the evidence map proves every requirement has a route, and the release gate is one script with its blockers named | Accepted 2026-09-06, amended by 0051 |
| [0042](0042-design-checklist-gate-and-the-human-checkpoint.md) | The design checklist is a gate with machine verdicts, and the beauty pass closes on a person's receipt, never on a pilot's | Accepted 2026-09-05 |
| [0043](0043-qa-profiles-carry-a-private-models-directory-and-the-gate-passes-on-what-the-code-does.md) | QA profiles carry a private models directory, the capability flags tell the truth, and the history drives read the seed | Accepted 2026-09-06 |
| [0045](0045-a-write-owns-its-fields-and-nothing-announces-what-it-did-not-store.md) | A write owns the fields its operation changes, and nothing announces what it did not store | Accepted 2026-09-06 |
| [0047](0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md) | Delivery guards the window itself, checks it again before typing, and never types a take twice | Accepted 2026-09-06 |
| [0046](0046-every-session-job-download-and-connection-has-one-owner-and-one-deadline.md) | Every session, job, download and connection has one owner and one deadline | Accepted 2026-09-06 |
| [0048](0048-speech-and-text-hold-under-test-silence-seams-stop-strings-tokens-numbers-and-notes.md) | Speech and text are never dropped by a shortcut: silence, seams, stop strings, protected tokens, numbers and notes hold under test | Accepted 2026-09-06 |
| [0049](0049-the-daemon-reports-what-runs-configuration-in-force-verified-models-and-the-backend-it-initialised.md) | The daemon reports what runs: the configuration in force, the models it verified and the backend it initialised | Accepted 2026-09-06 |
| [0050](0050-private-transfers-authenticated-adapters-and-isolated-qa-processes.md) | Private transfer modes, content-free logs, authenticated REST forwarding, safe Unicode parsing, truthful status and isolated QA processes | Accepted 2026-09-10, amends 0011, 0019, 0022, 0028 and 0043 |
| [0052](0052-qa-isolation-and-honest-assertions-every-check-proves-what-it-claims.md) | QA isolation and honest assertions: approved renders are held to colour and size, every host renders through the shared style check, Home and every settings section sit in the matrix, `mcp check` performs its handshake, and each scenario asserts the fact it names (amends 0020 and 0021) | Accepted 2026-09-06 |
| [0051](0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md) | The release gate passes on validated evidence bound to the tested binaries, and an unexplained failure stays a failure | Accepted 2026-09-06, amends 0030, 0034, 0039, 0040 and 0041 |
| [0053](0053-packages-plugin-and-ci-carry-what-the-binaries-need.md) | The packages, the plugin and CI carry what the binaries need: the Qt floor holds, the plugin speaks the contract, the lints see every edge | Accepted 2026-09-06 |
| [0054](0054-delete-duplicate-work-and-keep-one-owner.md) | Shared transport, ordered retention, requested startup work and one build path | Accepted 2026-09-10 |
| [0055](0055-completion-capture-and-client-actions-follow-authoritative-state.md) | Completion, capture and client actions follow authoritative state | Accepted 2026-09-10 |
| [0056](0056-live-desktop-actions-and-evidence.md) | Live desktop actions and summaries follow their data, controls remain reachable, and QA records running-window evidence | Accepted 2026-09-08 |
| [0057](0057-cuda-diarization-drop-in-with-cpu-fallback.md) | Optional CUDA diarization reuses the model set, reports the loaded provider and falls back to CPU when its prerequisite checks fail | Evaluated 2026-09-09; shipping blocked by performance, amends 0034 and 0035 |
| [0058](0058-strict-diarization-accuracy-evaluation.md) | Strict word-reference DER, reproducible scoring and predeclared held-out criteria govern diarization accuracy changes | Accepted 2026-09-09; product calibration in progress |
| [0059](0059-desktop-memory-budgets-follow-measured-reference-profiles.md) | Thor app RSS and anonymous memory use measured reference allowances with five fresh release launches per desktop mode | Accepted 2026-09-10; supersedes the app memory contract in 0020 |
| [0060](0060-independent-whisper-meeting-selection.md) | Independent Whisper meeting selection: `speech.meeting_model` names a Whisper model while dictation stays on its own provider | Accepted 2026-09-16 |
| [0061](0061-every-live-fragment-stays-the-stop-is-acknowledged-at-once-and-the-passes-say-they-are-queued.md) | Every live fragment stays on screen, the stop is acknowledged at once, the passes say they are queued, and `meetings.rename` renames a meeting from its title | Accepted 2026-09-16, amends 0035, 0036 and 0038 |
| [0062](0062-a-cut-off-analysis-answer-splits-the-part-and-the-reduce-joins-the-lists-in-code.md) | A cut-off analysis answer splits the part, and the reduce joins the lists in code | Accepted 2026-09-16, amends 0036 |
| [0063](0063-the-bar-meeting-timer-ticks-from-the-recording-anchor.md) | The bar's meeting timer ticks from the recording anchor, and a stage detail takes the room its row has | Accepted 2026-09-16, amends 0030 and 0061 |
| [0064](0064-a-release-publishes-itself-to-the-aur-and-the-plugin-mirror.md) | A release publishes itself to the AUR and the Omarchy plugin mirror | Accepted 2026-09-24 |
| [0065](0065-dettivo-for-linux-is-gpl-3-or-later.md) | Dettivo for Linux is licensed GPL-3.0-or-later | Accepted 2026-09-24 |
| [0066](0066-visual-checks-are-optional-not-a-gate.md) | Visual checks are optional tools; neither CI nor the release gate runs them | Accepted 2026-09-24 |
| [0067](0067-the-engines-compile-ggml-for-one-x86-64-baseline.md) | The engines compile ggml for one x86-64 baseline, never for the build machine's CPU | Accepted 2026-09-24 |
| [0068](0068-parakeet-ultra-is-a-catalogue-option.md) | Parakeet Ultra, Moondream's post-train of v3, is a catalogue option beside v2 and v3 | Accepted 2026-09-24 |
| [0069](0069-releases-ship-the-pacman-package-until-the-aur-account-exists.md) | Releases ship the pacman package; the AUR job waits for an account (registration closed) | Accepted 2026-09-25 |
| [0070](0070-a-release-is-local-qa-and-a-tag.md) | A release is local QA and a tag; CI tests, packages, clean-installs and publishes, and the deep checks are optional | Accepted 2026-09-25 |

## Writing a record

Copy [`template.md`](template.md). Open with what the decision does for the reader, then the situation that forced a choice, the choice, and its consequences including the bounds you know. Name mechanisms, paths and numbers; leave out adjectives. Plain hyphens, full sentences, active voice with the actor named.
