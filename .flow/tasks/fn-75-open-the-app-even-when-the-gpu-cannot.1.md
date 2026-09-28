---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-75-open-the-app-even-when-the-gpu-cannot.1 Implement Open the app even when the GPU cannot give it a window

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Every Dettivo Qt window (dettivo-app, dettivo-osd, and the runHost hosts dettivo-bar, dettivo-sheet, dettivo-insert-target) now handles QQuickWindow::sceneGraphError through one shared helper (qt/host/render_fallback.cpp). The first GPU scene-graph failure logs one warning naming Qt's error and execs the same binary once with QT_QUICK_BACKEND=software and DETTIVO_RENDER_FALLBACK. A second failure, or one in a process already drawing in software, exits 1 with a clear error and never restarts again. `[app] renderer = "auto" | "software"` is in the schema, the default file, the fixtures, Settings / General / Display and docs/config.md, and a user-set QT_QUICK_BACKEND wins over it. `dettivo app status` carries environment.renderer_reason, and `dettivo doctor` gains an `app` block and row. ADR 0077 records the decision.

R-ID coverage:
- R1, R2, R3, R5: the `<host>-render-fallback` ctest (qt/host/render_fallback_test.sh), registered for all five hosts through dettivo_add_render_test. It runs `--render` on a private Xvfb with QT_XCB_GL_INTEGRATION=none and checks four cases. (1) The fallback: one warning, one restart, a PNG. (2) The guard: DETTIVO_RENDER_FALLBACK already set exits 1 with no restart. (3) The config: renderer = "software" means no restart. (4) The environment: QT_QUICK_BACKEND beats the file. With the guard call removed, the test failed with exit 134 (SIGABRT), the reported crash.
- R3 settable from Settings: settings_keys.cpp, GeneralSection.qml, sample_settings.cpp; lint-settings-keys is green.
- R4: app_environment.h renderer_reason; crates/dettivo-cli/src/app.rs doctor_facts plus its unit test; the doctor golden and a human-row assertion in crates/dettivo-cli/tests/doctor.rs.
- R6: docs/adr/0077-a-window-the-gpu-cannot-draw-restarts-once-on-the-software-renderer.md.

Gate: `just build test lint` exited 0 on the committed tree, run with VK_ICD_FILENAMES/VK_DRIVER_FILES=/nonexistent. Baseline: green before any edit. Without the Vulkan override, dettivod's meetings_diarize_crash test fails on this machine. It falls through to the installed /usr/lib/dettivo/engines/dettivo-engine-whisper on Vulkan, and that engine crashes while the GPU is full (23.4 of 24.5 GB). This is a pre-existing test-isolation gap that this diff does not touch. Suggested follow-up: make that test independent of the installed engine.

Incident, repaired: the first version of the test started Xvfb with -displayfd. Xvfb then took display :0 and unlinked the desktop XWayland's /tmp/.X11-unix/X0, so new X11 clients could not connect. I restored the path with a symlink (X0 -> X0_, XWayland's other listener), and xprop on :0 answered again. The test now picks an explicit free display from :100 to :199 and skips any display that has a socket or a lock.

Xvfb is now a build prerequisite: check-toolchain, the CI setup action and the README.

stage: impl-review - skipped(config: REVIEW_MODE=none)
Tier: implementer claude-opus-5-5 (project routing block)
## Evidence
- Commits: f18db0e41dd09b1f33e1552366cc4477a65f37cd
- Tests: just build test lint, ctest --test-dir build/qt -R render-fallback
- PRs: