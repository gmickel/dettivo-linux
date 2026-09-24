# OSD: the QML pill component, dettivo-osd layer-shell host and frame pacing

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-10 row: "`osd-qml-component-layer-shell-host` | phase 1 | depends on S-07, S-34 | FR-O1 to O3, FR-V6, T11, NFR-12; baseline `docs/design/studio/baselines/osd.png` | Qt Quick Test for OSD states; Xvfb drive asserts OSD labels; visual diff against the OSD baseline; frame timing on Thor"
> masterplan FR-O1: "The OSD is one QML component (recording pill with level, transcribing, inserted, error) in the shared Dettivo QML module."
> masterplan FR-O2: "On Omarchy the component is hosted by the Dettivo shell plugin as a `panel`, shown and hidden from the daemon's event stream, so no separate OSD process runs."
> masterplan FR-O3: "Elsewhere `dettivo-osd`, a small Qt process using layer-shell-qt, hosts the same component on wlroots compositors and KDE, positioned per setting; on compositors without layer-shell it falls back to a normal undecorated always-on-top window or is disabled with a notice."
> masterplan FR-V6: "The OSD is the signature surface: a compact pill with a real-time waveform rendered on the scene graph (no Canvas repaints), a transcribing state, an inserted state that shows the first words, and an error state that says what to do. It never covers the caret area of the focused window on Hyprland and honours a per-monitor position setting."
> masterplan 8.10 OSD baseline: "One 36 px pill, six states: Listening (accent dot, live bars, hint), Transcribing (dimmed bars, engine and elapsed), Enhancing (gold thread and the raw first words), Inserted (accent border, target app in accent, first words), Copied (clipboard icon, reason and shortcut), Error (urgent dot, device name, action in urgent) ... The pill never shows more than one sentence of text and never a spinner. State is carried by the dot colour and the bars."
> masterplan NFR-12: "Zero dropped frames over a 10 s recording animation at 60, 120 and 144 Hz on Thor; OSD render cost ≤ 1 ms per frame"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

The pill is what the user sees of Dettivo while dictating. This spec ships it as one QML component in the shared `Dettivo` module, `Dettivo.Osd`, with the six states of the approved baseline driven entirely by the daemon's event stream (`dictation.state`, `audio.level`, insertion outcome), and the `dettivo-osd` process that hosts it outside Omarchy on a layer-shell surface positioned per monitor and setting, falling back to an always-on-top window where layer-shell is absent. The waveform is scene-graph geometry fed by the level meter, never a Canvas repaint, and the whole pill costs under a millisecond a frame. The Omarchy shell plugin (S-21) later hosts the same component as a panel, so the component owns every state and the host owns only placement. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `qt/qml/Dettivo/components/Osd.qml` (plus `OsdBars.qml`, `OsdDot.qml`): a 36 px pill with `state` in `listening | transcribing | enhancing | inserted | copied | error | hidden`, `level` (0..1 from `audio.level`), `hint`, `engine`, `elapsed`, `words` (first words, one sentence at most), `target`, `reason`, `action`. Colours, type and motion come from the theme object of S-34; the bars are a `QSGGeometryNode`-backed item (`OsdBars` in C++ under `qt/host`) that updates from the level with no Canvas; the dot colour and the bars carry the state; transitions are the module's motion tokens; every text is one line and elides. Accessible name and role per element (QA-13). [paraphrase]
- `OsdModel` (C++ in `qt/host`, shared by `dettivo-osd` and later the plugin): connects to the daemon, subscribes to `dictation.state`, `audio.level`, `job.progress` and `engine.state`, maps them to the component's properties (the insertion outcome and target come from the `dictation.state` payload's completion fields, the first words from the transcript), auto-hides `inserted` and `copied` after `[osd] hide_after_ms` and `error` after its own delay, and reconnects when the daemon restarts. [inferred]
- `dettivo-osd` (`qt/apps/dettivo-osd`): a Qt process using `layer-shell-qt` (`LayerShellQt::Window` with layer `overlay`, anchors per `[osd] position` (`top`, `bottom`, `top_left` ... `bottom_right`), margin per `[osd] margin`, exclusive zone 0, keyboard interactivity none) on wlroots compositors and KDE; on a compositor without `zwlr_layer_shell_v1` it falls back to a frameless `Qt.WindowStaysOnTopHint` tool window at the same position, and when neither works it exits 0 with a notice in the log and the doctor. `[osd] monitor` names the output (`focused` follows the focused output through the daemon's `insert.target` monitor hint on Hyprland, else the primary). Never covers the caret area: on Hyprland the host asks the daemon for the focused window geometry (S-08's probe gains `geometry`) and moves the pill to the opposite vertical edge when the window's caret region would be covered. [paraphrase]
- Lifecycle: `dettivo-osd` is a user service (`dettivo-osd.service`, `WantedBy=graphical-session.target`), started by the packaging of S-01 only when the Omarchy plugin is not installed; `[osd] enabled = true` and `dettivo osd show|hide|status` for scripting; in QA mode `DETTIVO_E2E_OSD_STATE=<state>` shows a state without a daemon for the visual baseline. [inferred]
- Frame pacing: the recording animation runs on the render thread from the level meter events (`audio.level` at 30 Hz), interpolated per frame by the scene-graph node; the drive measures dropped frames with `QSG_RENDER_TIMING` output parsed by the QA rig on the development machine at the monitor's refresh rate, and CI under Xvfb asserts labels and states only. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- No new daemon methods; the OSD consumes `events.subscribe` topics from S-07 and S-06. The `dictation.state` payload gains the Linux completion fields `insertion` (outcome, method, backend, target app) and `first_words` so the pill shows them without a second request, recorded in `docs/api/linux-deltas.md`. [inferred]
- `insert.target` gains `geometry` (x, y, width, height, output) when the probe knows it (Hyprland IPC, X11), recorded as a delta. [inferred]
- CLI: `dettivo osd show <state> | hide | status` talks to `dettivo-osd` over its own small control socket at `$XDG_RUNTIME_DIR/dettivo/osd.sock` (one line JSON); `status` reports host kind (`layer_shell`, `window`, `disabled`), position and monitor. [inferred]
- Config (`docs/config.md`): `[osd] enabled = true`, `position = "top"`, `margin = 24`, `monitor = "focused"`, `hide_after_ms = 1800`, `error_hide_after_ms = 4000`, `show_level = true`. [inferred]
- Design: the component matches `docs/design/studio/baselines/osd.png` state by state on every Omarchy theme; the visual diff of S-03's rig compares `dettivo-osd` renders under Xvfb with the approved crops at the perceptual threshold. [paraphrase]

## Edge Cases & Constraints

- Daemon not running: the OSD stays hidden and reconnects with backoff; no error pill for a missing daemon. [inferred]
- Overflow (`events.overflow`): the OSD resubscribes and re-reads `dictation.status` instead of showing stale state. [inferred]
- Two OSD hosts (plugin and `dettivo-osd`): `dettivo-osd` exits with a notice when the Omarchy plugin advertises itself (a name on the session bus), so one pill shows. [inferred]
- Text longer than one sentence: the first sentence, elided to the pill width; never a spinner. [paraphrase]
- Reduced motion (`[osd] motion = "reduced"` or the theme's flag): bars hold a static level and transitions are cuts. [inferred]
- Multi-monitor: the pill follows `[osd] monitor`; an output that disappears moves the pill to the primary. [inferred]

## Acceptance Criteria

- **R1:** Qt Quick Tests for `Dettivo.Osd` cover the six states and hidden: property to visual mapping (dot colour, bars dimmed or live, border accent, icon), one-line elision, hide timers, reduced motion, and accessible names for every element; the tests run in `just test-qt` and CI `unit-qml`. Errors: an unknown state name is rejected with a warning and the pill hides. [paraphrase]
- **R2:** `dettivo-osd` under Xvfb in the CI drive job: a dictation through the mock microphone shows Listening, Transcribing and Inserted in order (the drive asserts the labels through AT-SPI on both drivers) and the pill hides after `hide_after_ms`; an insertion refused by the guards shows Copied or Error with the reason. Errors: as stated. [paraphrase]
- **R3:** Visual regression: renders of the six states under `DETTIVO_E2E_OSD_STATE` on the default theme and on a light theme match the crops of `osd.png` within the perceptual threshold; the diff runs in CI and blocks on an unapproved difference. Errors: a missing baseline crop fails the job by name. [paraphrase]
- **R4:** Hosting: on the development machine `dettivo-osd` creates a layer-shell overlay surface at the configured position and monitor, moves away from the focused window's caret region on Hyprland, and `dettivo osd status` reports `layer_shell`; with layer-shell forced off (`[osd] host = "window"`) the fallback window shows the same pill; with both unavailable the process exits 0 with a notice and the doctor names it. Errors: as stated. [paraphrase]
- **R5:** Frame pacing on the development machine: the drive's 10 s recording animation reports zero dropped frames at the monitor's refresh rate and a render cost under 1 ms per frame from `QSG_RENDER_TIMING`, written into the evidence directory; the scene-graph bars item never triggers a Canvas repaint (asserted by the absence of `QQuickPaintedItem` in the component and a render-thread trace). Errors: a dropped frame is reported with its timestamp. [paraphrase]
- **R6:** `docs/osd.md` describes the states, hosts and settings; the `[osd]` keys are documented and printed by `config print-default`; the user service and the `dettivo osd` verbs are documented; `docs/design/README.md` maps the component to its baseline. Errors: as stated. [paraphrase]

## Boundaries

- No Omarchy shell plugin or bar glyph; S-21 hosts the component as a panel and adds the 16 px glyph. [paraphrase]
- No meeting states or the elapsed timer; S-23 extends the component. [paraphrase]
- No Enhancing pipeline; the state exists in the component and is driven when the polish spec lands. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- The OSD is the signature surface and the only UI the flagship desktop shows during dictation; hosting the same component in a process elsewhere and in the plugin on Omarchy keeps one design and zero extra resident processes there (T11). [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** the pill is the approved baseline on every theme and the flagship desktop hosts it without a separate process. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the drive asserts every state of a dictation and the frame timing is evidence, not a claim. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
| R2 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
| R3 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
| R4 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
| R5 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
| R6 | fn-12-osd-the-qml-pill-component-dettivo-osd.1 |
