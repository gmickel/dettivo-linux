# 0015. One pill component, hosted on a layer-shell overlay with a window fallback

Status: Accepted 2026-09-04. The visual check and the pacing measurement are generalised by [0021](0021-visual-regression-gate-and-frame-pacing.md): the check is `dettivo-qa visual` over every surface, theme and scale, and the collector lives in `qt/host/pacing` for every Qt binary.

## What this gives you

The recording pill looks and behaves the same whether the Omarchy panel hosts it or `dettivo-osd` does, it never covers the window you are dictating into on Hyprland, and every state of it is proven three ways: Qt Quick Tests on the component, a drive that reads its labels through the accessibility tree, and a visual check against the approved baseline on a dark and a light theme.

## Situation

The approved baseline (`docs/design/studio/baselines/osd.png`) fixes six states of one 36 px pill. On Omarchy the shell plugin will host the pill as a panel (S-21), so the states and the timers cannot live in a process. Off Omarchy a process has to host it, and compositors differ: wlroots compositors and KDE offer `zwlr_layer_shell_v1`, GNOME does not, and CI runs under Xvfb. The daemon's `dictation.state` payload carried ids and states only, so the pill had no way to name the target app or show the first words without a second request. The insertion probe of S-08 is not merged, so the focused window's geometry has no daemon route yet. Fast engines finish a short take in under 100 ms, which makes a Transcribing state unreadable. The scene graph's software adaptation, which the offscreen renders use, draws rectangle nodes but not arbitrary geometry nodes.

## Decision

- `Dettivo.Osd` in the shared module owns the states, the hide timers, the elision and the accessible names; a host binds `state` and the texts and owns the window only. The bars are `OsdBars`, one C++ `QQuickItem` in the module whose node is a parent node with one rectangle node per bar, interpolated per frame on the render thread; the item requests its next frame from `updatePaintNode`, so the animation runs at the display rate with no timer. No `Canvas`, no `QQuickPaintedItem`; `scripts/lint-qml-tokens.sh` refuses both.
- The state model (`OsdModel`) and the daemon link live under `qt/host/osd/` for `dettivo-osd` and the panel alike. `dictation.state` gains `insertion` (outcome, method, backend, target app, reason) and `first_words` on the completion transition, recorded in `docs/api/linux-deltas.md`; `first_words` is the transcript's first sentence bounded to 72 characters, the one place a payload carries transcript text, and the daemon's logs still never do.
- `dettivo-osd` picks its host at start: `layer_shell` when the session is Wayland, the binary was built with layer-shell-qt and a fresh `wl_registry` roundtrip shows `zwlr_layer_shell_v1`; `window` (frameless, always on top, no focus) otherwise; `disabled` with a notice in `osd.status.json` beside the sockets when `[osd] enabled = false` or `host = "layer_shell"` cannot be met, or when the panel plugin's name `dev.dettivo.OmarchyPanel` is on the session bus. `[osd] host` forces a host.
- The focused window's geometry comes from Hyprland's own IPC socket inside the host rather than through the daemon, because the probe that would carry it (`insert.target` `geometry`) is not on `main`; when the probe lands, the host reads the daemon's answer and the IPC path goes. The rule: the pill leaves its configured edge when the focused window reaches into the pill's band on that edge and not into the band on the opposite edge.
- Transcribing holds for at least one second (`OsdModel::kMinTranscribingMs`); a completion that arrives sooner waits for the beat. A pill state a person cannot see is no state, and the drives read it through the slower driver in that window.
- `dettivo osd show|hide|status` and the drives talk to the pill over `osd.sock` beside the daemon socket, one JSON line each way, with no daemon in the path. `DETTIVO_E2E_OSD_STATE` shows a state with the baseline's sample text and no daemon.
- The visual check compares each render with its crop by structure: both images reduce to an 18-row grid, each cell scored by contrast against the image's own background and by saturation, and the grids compare by correlation with the candidate stretched to the baseline's width. The threshold is 0.55: a render in another monospace font scores 0.59 and up against its own crop, an unrelated layout 0.45 and under. The crops are cut from `osd.png` by `scripts/design/crop-osd-baseline.py` and re-cut after a re-approval.
- Frame pacing is measured by `dettivo-osd --pacing` on the real display: swap timestamps on the render thread, render durations between `beforeRendering` and `afterRendering`, a dropped frame being a swap interval of two full refresh periods or more. `dettivo-qa osd-pacing` writes the summary and the `QSG_RENDER_TIMING` trace into the evidence directory and passes on zero dropped frames with the 99th-percentile render cost under a millisecond.

## Consequences

- On the development machine (Hyprland, 240 Hz, scale 1.25) the pill renders in 0.015 ms a frame on average, 0.04 ms at the 99th percentile, with 2 310 to 2 330 frames delivered per ten seconds; runs show zero to two swap intervals of two periods or more, which the trace attributes to the compositor's frame callbacks on a busy desktop rather than to the pill's render cost. The measurement is per machine and per run, so it is evidence, not a CI gate; CI proves the states under Xvfb.
- Under a Wayland compositor without layer shell the compositor places the window; a tiling compositor may tile it unless a rule floats `Dettivo OSD`. `layer_shell` covers every compositor the strategy names.
- Two sockets share the daemon's directory (`dettivo.sock`, `osd.sock`), so a QA profile isolates the pill through `DETTIVO_IPC_SOCKET` alone, and `dettivo-osd` ends on SIGTERM so its socket goes with it.
- The plugin's bus name is fixed here before the plugin exists; S-21 claims it.
- The one-second Transcribing beat delays the Inserted confirmation by up to a second after a fast engine; the text is already in the target application by then.

## Speech-level response (2026-09-23)

Quiet speech visibly moves the recording bars. Both hosts map microphone RMS logarithmically from -60 dBFS at the floor to 0 dBFS at full scale, then use the existing bar envelope and smoothing. Silence and signals below the floor remain still. The Omarchy pill previously used unscaled RMS while the standalone host multiplied it by four; both now use the same display curve. Capture gain, audio, recognition, and the separate bar widget meter are unchanged.
