# Open the app even when the GPU cannot give it a window

## Goal & Context

On 2026-09-28 the installed Dettivo 0.3.0 app would not open. Diablo IV held 16.7 GB of the RTX 4090's 24.5 GB, other programs held most of the rest (23.6 GB in use), and `dettivo-app` aborted at startup every time: Qt Quick's scene graph could not create its GPU resources, and Qt's default reaction to a scene-graph error is `qFatal`, so the process dumped core (SIGABRT in `QMessageLogger::fatal` from libQt6Quick, called from `QWindow::event`). The app simply never appeared, with no message. Launching it with `QT_QUICK_BACKEND=software` worked at once.

A busy or unavailable GPU (a game, a GPU reset, a driver update, a remote session) must not stop the user from opening Dettivo. This spec makes every Dettivo Qt surface fall back to Qt's software renderer when GPU rendering fails, and say why.

## Approach

- Handle `QQuickWindow::sceneGraphError` on every top-level Qt Quick window in the app, the OSD pill and the other Dettivo Qt executables that show windows (`qt/apps/*`). A connected handler replaces Qt's `qFatal`.
- On a scene-graph error, fall back to the software renderer. The simplest robust route is to re-launch the same executable once with `QT_QUICK_BACKEND=software` (and a guard variable so it never loops), because the graphics API cannot be switched inside a process once windows exist. Log the error at warn level with the reason.
- Make software rendering configurable: `config.toml` gains a key such as `[app] renderer = "auto" | "software"`, and `QT_QUICK_BACKEND` set by the user still wins.
- `dettivo doctor` / `dettivo app status` report the renderer the app is using and why, so a fallback is visible.

## Quick commands

- `just build test lint`

## Acceptance

- **R1:** When Qt Quick cannot create its GPU scene graph, `dettivo-app` opens its window with the software renderer instead of aborting, and logs one warning naming the error. The same holds for `dettivo-osd` and every other Dettivo Qt executable that opens a window.
- **R2:** The fallback happens at most once per start: a software-rendered process that also fails exits with a clear error, never a relaunch loop.
- **R3:** `[app] renderer = "software"` in `config.toml` forces the software renderer from the start; `auto` (the default) keeps today's GPU path. A user-set `QT_QUICK_BACKEND` is respected. The key is documented in `docs/config.md` and settable from Settings.
- **R4:** `dettivo app status` (and `dettivo doctor`) report the renderer in use and, after a fallback, the reason.
- **R5:** A test proves the fallback without a real GPU failure, e.g. by forcing a scene-graph error through a test hook or an unusable graphics API in an offscreen/Xvfb session, and checks the window opens and the relaunch guard holds.
- **R6:** An ADR records the decision.

## Boundaries

- No change to the speech or diarization engines' GPU use.
- No attempt to free or manage other programs' GPU memory.

## Decision Context

Found on 2026-09-28 when the app would not open while a game and a benchmark filled the GPU. Gordon asked to capture and fix it.
