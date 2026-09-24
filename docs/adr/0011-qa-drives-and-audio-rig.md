# 0011. Desktop drives under X11 with an in-repo fallback, a virtual audio rig and contract fixtures

Status: Accepted 2026-09-03

Amended by [0050](0050-private-transfers-authenticated-adapters-and-isolated-qa-processes.md), which clears inherited QA environments and provides private cache storage.

## What this gives you

Every user-facing behaviour has an automated way to prove it works on a real desktop, including hotkeys, capture, insertion and the GUI, and the proof keeps running in CI without a physical microphone or a person at the keyboard.

## Situation

cua-driver 0.20.0 on Hyprland lists and drives XWayland windows only, walks AT-SPI trees only for windows it can list, and injects input through XTEST; its Wayland path needs the RemoteDesktop portal, which `xdg-desktop-portal-hyprland` 1.4.1 does not implement. The Windows port lost weeks when its computer-use harness broke and shipped on hand-rolled window probes. PipeWire can create null sinks and play fixture audio into them.

## Decision

QA mode is product code, switched by environment variables that reuse the macOS names (`DETTIVO_MOCK_MODE`, `DETTIVO_E2E_OPEN`, `DETTIVO_E2E_SEED` and the rest). The Qt binaries run under `QT_QPA_PLATFORM=xcb` with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` in QA, on XWayland locally and Xvfb in CI, so cua-driver can enumerate and drive them. `dettivo-qa` defines a driver interface with two implementations, cua-driver and an in-repo AT-SPI driver with XTEST input, and every scenario runs on both. Real capture paths are exercised through null sinks (`pactl load-module module-null-sink`) fed by `pw-play`. Contract fixtures replay every method and error over the socket. Semantic routes (CLI, socket, database) are used for every postcondition that is not a GUI outcome.

## Consequences

- Wayland-only behaviour (layer-shell OSD, virtual keyboard insertion) is proven by daemon-level tests and `hyprctl`-driven checks on the development machine rather than by cua.
- The bar widget is verified by screenshot and IPC state because Quickshell exposes no accessibility tree.
- Every scenario runs in an isolated profile with a private models directory (only the named test models hard-linked in, never a link to the user's directory; [ADR 0043](0043-qa-profiles-carry-a-private-models-directory-and-the-gate-passes-on-what-the-code-does.md) superseded the linked-in real directory this record first described), and first-run profiles are used on purpose to expose contract gaps.
- cua-driver is pinned per release and its Wayland support re-checked at each upgrade.
