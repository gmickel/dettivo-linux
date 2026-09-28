# 0077. A window the GPU cannot draw restarts once on Qt's software renderer

Status: Accepted 2026-09-28

## What this gives you

Dettivo opens while a game or another program holds all of the GPU's memory. The app, the recording pill and every other Dettivo window restart themselves once on Qt's software renderer and draw on the CPU, where they used to abort with no message. `[app] renderer = "software"` in `config.toml` skips the GPU from the start, and `dettivo app status` and `dettivo doctor` say which renderer the app uses and why.

## Situation

On 2026-09-28 the installed Dettivo 0.3.0 app would not open. Diablo IV held 16.7 GB of the RTX 4090's 24.5 GB and other programs held most of the rest. Qt Quick could not create its scene graph's GPU resources, and Qt's reaction to a scene-graph error with no handler is `qFatal`, so `dettivo-app` died with SIGABRT in `QMessageLogger::fatal`, called from `QWindow::event`. `QT_QUICK_BACKEND=software` opened the same app at once.

Qt emits `QQuickWindow::sceneGraphError` in place of the `qFatal` when a slot is connected. The graphics API cannot change inside a process once its first window exists (`QQuickWindow::setGraphicsApi` works only before), so the failed window cannot be rebuilt in software in place.

Two ways to restart were on the table. Spawning a new process and exiting would change the process id, and systemd would treat `dettivo-osd.service`'s main process as gone. `execv` of the same binary keeps the process id and replaces everything else.

## Decision

- **One shared helper.** `qt/host/render_fallback.cpp` holds the whole mechanism. Every host calls `renderer::choose` before its `QGuiApplication` exists and `renderer::guard` on its window: `dettivo-app`, `dettivo-osd`, and the hosts of `runHost` (`dettivo-bar`, `dettivo-sheet`, `dettivo-insert-target`).
- **The choice at start.** In order: a process that carries `DETTIVO_RENDER_FALLBACK` is the restart and draws in software; a user-set `QT_QUICK_BACKEND` is left to Qt; `[app] renderer = "software"` calls `QQuickWindow::setGraphicsApi(Software)`; `auto`, the default, leaves Qt's GPU path as it was. A value other than `auto` or `software` is `auto` with one warning. The key sits under `[app]` and applies to every Dettivo window, because the pill fails on the same GPU the app does.
- **The restart.** The first scene-graph error prints one warning naming Qt's message, sets `QT_QUICK_BACKEND=software` and `DETTIVO_RENDER_FALLBACK=<message>`, and `execv`s `/proc/self/exe` with the original arguments. The app's instance lock and every socket are close-on-exec, so the restarted process takes the lock and the socket again.
- **At most once.** A process that is the restart, or already draws in software, and still gets a scene-graph error prints `cannot draw the window with the software renderer either: <message>` and exits 1. The restarted process removes `DETTIVO_RENDER_FALLBACK` from its environment after reading it, so a program it starts is never mistaken for a restart.
- **Reported.** The app's status carries `environment.renderer` (`OpenGL`, `Vulkan`, `Software`) and `environment.renderer_reason` (`default`, `config: [app] renderer = software`, `environment: QT_QUICK_BACKEND=<value>` or `fallback: <Qt's message>`). `dettivo doctor` prints both on its `app` row. A process whose reason is not `default` also logs it at start.

## Consequences

- The software renderer draws on the CPU. No Dettivo surface uses a shader effect, so the windows look the same; their animations cost CPU time instead of GPU time, and that cost is not measured here.
- A restart repeats the process's start-up, about as long as the first one. The pill first shows its window when a dictation starts, so a pill that falls back restarts at that moment, misses that first show, and reads the dictation state from the daemon again when it reconnects.
- The restart replaces the process image, so nothing the failed process held in memory survives. At start-up that is nothing the user made.
- `/proc/self/exe` fails to exec when the binary was deleted after start (an upgrade under a running process). The process then prints why and exits 1, the same end as a second failure.
- The test `<host>-render-fallback` (`qt/host/render_fallback_test.sh`) runs each host's `--render` on a private Xvfb with `QT_XCB_GL_INTEGRATION=none`. Qt Quick then has no OpenGL, which aborted a host with SIGABRT before this change. It checks one warning, one restart and a written picture; exit 1 without a restart when `DETTIVO_RENDER_FALLBACK` is already set; no restart under `renderer = "software"`; and that `QT_QUICK_BACKEND` wins over the file. `just test` therefore needs Xvfb, which `scripts/check-toolchain.sh` names and the CI setup installs.
- The speech, language and diarization engines use the GPU exactly as before; this record changes only the Qt windows.
