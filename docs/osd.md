# The recording pill

While you dictate, one 36 px pill tells you what Dettivo is doing: the bars follow your voice while it listens, they freeze and dim while the engine works, and the pill then names where the words went and shows the first of them, or says in one sentence what went wrong and what to do next. The pill is one QML component, `Dettivo.Osd`, driven entirely by the daemon's event stream; on Omarchy the shell plugin will host it as a panel, everywhere else the `dettivo-osd` process hosts it on a layer-shell overlay or, where the compositor has none, in a small always-on-top window. The approved design is `docs/design/studio/baselines/osd.png`, and every render is checked against it.

## The states

| State | What you see | When |
|---|---|---|
| Listening | accent dot, live bars, `Listening`, the hint `release to insert`; during a meeting the hint is the meeting's elapsed time (`12:04`), counting on from the duration `meeting.state` carried | `dictation.state` is `recording`, or `meeting.state` is `recording` ([docs/meetings.md](meetings.md)); the bars follow `audio.level` from either track |
| Transcribing | held dot, frozen dimmed bars, `Transcribing`, the engine and the elapsed time | `transcribing` and `inserting`; shown for at least one second so a fast engine never makes it flicker |
| Enhancing | a travelling gold thread and the raw first words | reserved for the polish spec; the state exists in the component |
| Inserted | accent border, the insert icon, `Inserted into` and the target app in accent, the first words | the completion carries `insertion.outcome = inserted` |
| Copied | the clipboard icon, `Copied to clipboard`, the reason and `Ctrl+V` | `copied_to_clipboard` |
| Error | urgent dot and border, a title such as `No microphone`, the reason, the action in urgent | a `failed` session, a failed insertion, a silent take (`Nothing heard`), or a `failed` meeting (`Meeting failed`, the reason from the event) |

Inserted and Copied hide after `hide_after_ms`, Error after `error_hide_after_ms`, Listening and Transcribing when the session moves on. A cancelled session hides the pill, and so does a meeting's stop: the finalisation that follows a meeting is the app's and the CLI's to show, not the pill's. Every text is one line and elides; the first words are the transcript's first sentence, at most 72 characters. The pill never shows a spinner: the state is carried by the dot colour and the bars.

## Hosting

`dettivo-osd` is a user service, `dettivo-osd.service`, wanted by `graphical-session.target`; the packaging enables it where the Omarchy plugin is not installed, and the process exits with a notice when the plugin advertises itself on the session bus (`dev.dettivo.OmarchyPanel`), so one pill shows. On Omarchy the plugin's panel hosts the same component and holds that name through `dettivo osd host-panel` while `[omarchy] osd = "panel"` ([docs/omarchy.md](omarchy.md)); `dettivo osd status` then answers from the notice the process left.

| Host | Where | How to tell |
|---|---|---|
| `layer_shell` | Hyprland, sway and the other wlroots compositors, KDE: a `zwlr_layer_shell_v1` overlay surface anchored per `position` with `margin` from the edges, exclusive zone 0, no keyboard focus | `dettivo osd status` reports `host: layer_shell` |
| `window` | Everything else, and X11 or Xvfb: a frameless, always-on-top tool window titled `Dettivo OSD`; on X11 it sits at the configured position, on a Wayland compositor without layer shell the compositor places it (a tiling compositor may need a rule for the title) | `host: window` |
| disabled | `enabled = false`, or `host = "layer_shell"` on a session that has none: the process exits 0 and leaves a notice | `dettivo doctor` prints `osd  disabled: <notice>` |

`monitor = "focused"` follows the focused output on Hyprland through its IPC socket and takes the primary screen elsewhere; a named output that is not connected falls back to the primary. On Hyprland the pill also keeps clear of the focused window's caret region: when that window reaches into the pill's band on the configured edge and leaves the opposite edge free, the pill takes the opposite edge for that show, and returns when the next show finds the edge free (`dettivo osd status` reports the edge in use as `position`).

The daemon may come and go: the pill reconnects with a backoff from half a second to eight, shows nothing while the daemon is away, re-reads `dictation.status` when it is back, and re-subscribes after an `events.overflow` instead of showing a stale state.

## Settings

Everything lives in `[osd]` of `config.toml` ([docs/config.md](config.md)); `dettivo config print-default` prints the section with its comments.

| Key | Default | Meaning |
|---|---|---|
| `enabled` | `true` | Off: `dettivo-osd` exits with a notice. |
| `host` | `"auto"` | `auto` takes the layer shell where the compositor offers it and the window otherwise; `layer_shell` or `window` force one. |
| `position` | `"top"` | `top`, `bottom`, `top_left`, `top_right`, `bottom_left`, `bottom_right`. |
| `margin` | `24` | Pixels from the anchored edges. |
| `monitor` | `"focused"` | The focused output on Hyprland, else the primary; or an output name such as `DP-3`. |
| `hide_after_ms` | `1800` | How long Inserted and Copied stay. |
| `error_hide_after_ms` | `4000` | How long Error stays. |
| `show_level` | `true` | The bars follow the live level; off holds them at the floor. |
| `motion` | `"full"` | `reduced` holds the bars still and cuts between states; `full` follows the desktop's reduced-motion preference. |

## From the command line

```
dettivo osd status                          # host, position, monitor, what the pill shows, daemon link, theme
dettivo osd show listening                  # any state, with the baseline's sample text
dettivo osd show error --title "No microphone" --reason "Arctis Nova disconnected" --action "choose input"
dettivo osd show inserted --target ghostty --words "Add a regression test"
dettivo osd hide
```

The verbs talk to `dettivo-osd` directly over `osd.sock` beside the daemon socket (`$XDG_RUNTIME_DIR/dettivo/osd.sock`, or next to `DETTIVO_IPC_SOCKET`), one JSON line each way, so they work while the daemon is down. An absent pill exits 2 naming `dettivo-osd.service`; an unknown state exits 4 with the accepted names. `dettivo doctor` has an `osd` line with the same facts.

## Proving it

- `just test-qt` runs the Qt Quick Tests for the component (`qt/qml/Dettivo/tests/qml/tst_osd.qml`: the six states and hidden, elision, the hide timers, reduced motion, accessible names, an unknown state), the host tests (`qt/host/osd/osd_host_test.cpp`: the event stream to state mapping, reconnect and overflow, the control protocol, the settings, caret avoidance) and the `dettivo-osd` process tests (the disabled and layer-shell-only notices, the QA variable refusals).
- The visual regression job `just qa-visual` ([docs/qa.md](qa.md)) renders the six states at 1x and 2x on every theme fixture and both built-in palettes under `DETTIVO_E2E_OSD_STATE` and compares each with its baseline under `docs/design/studio/baselines/osd/` using `dettivo-visual-diff`: the Black Gold crops cut from `osd.png` by `scripts/design/crop-baseline.py`, the approved renders for the other themes and scales; the rig workflow's `visual` job blocks the nightly and release runs on an unapproved difference (ADR 0040), and a missing crop fails by name. The CTest case `dettivo-osd-render` proves the render and the negative style check over the pill's tree.
- The drive `osd_dictation` (`just qa-drive osd_dictation atspi|cua`) dictates the speech fixture through the mock microphone and reads `Listening`, `Transcribing` and `Inserted` through the accessibility tree on both drivers, waits for the pill to hide, then runs a take without the mock backend (`DETTIVO_MOCK_INSERT=0`, `[insert] backend = "clipboard"`) and reads `Copied to clipboard` or `Not inserted` with its reason; the pill's frame pacing over both takes is recorded through `DETTIVO_QA_PACING` and judged while the bars animate. The drive `theme_switch` replaces the theme directory under the pill and the app and reads when the palette applied. CI runs both on both drivers under Xvfb.
- `just qa-osd-pacing` runs the listening animation for ten seconds on the real display with `QSG_RENDER_TIMING` on and writes `pacing.json` and the render-thread trace under `qa-evidence/`: frames, the refresh rate, every interval of two periods or more with its timestamp, and the render cost per frame. The collector is the shared one every Qt host carries (`qt/host/pacing`). The bars are one scene-graph item (`OsdBars`, rectangle nodes under one parent node); nothing in the pill uses `Canvas` or `QQuickPaintedItem`, and `scripts/lint-qml-tokens.sh` refuses either.

`DETTIVO_E2E_OSD_STATE=<state>` (with `DETTIVO_QA_MODE=1`) shows one state with the baseline's sample text and no daemon, which is how the renders and the pacing run get their pill.
