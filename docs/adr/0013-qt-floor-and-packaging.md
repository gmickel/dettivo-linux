# 0013. Qt 6.8 floor and AUR packages with engine binaries as separate files

Status: Accepted 2026-09-03, amended 2026-09-06 by 0053 (the floor is enforced in the source by scripts/lint-qt-floor.sh)

## What this gives you

`yay -S dettivo-bin` installs the whole product on Omarchy with the dependencies the desktop already has, and a later CUDA package replaces the engine binaries without touching the daemon or the app.

## Situation

Omarchy runs Qt 6.11 and ships `qt6-base`, `qt6-declarative`, `qt6-wayland` and `layer-shell-qt`. Debian 13, Fedora, openSUSE and Arch package Qt 6.8 or newer; Ubuntu 24.04 LTS ships 6.4. Omakade sets a 6.8 floor. The ggml engines are separate binaries by design (ADR 0003).

## Decision

Qt 6.8 is the minimum. AUR `dettivo` builds from source and `dettivo-bin` repackages the GitHub release tarball; both install the daemon, the engine binaries, the CLI, the app, the OSD, the QML module, systemd user units, the desktop entry, icons, shell completions and `NOTICE.md`. The default build ships every engine with Vulkan and CPU backends. Engine binaries are discovered by name so a `dettivo-engines-cuda` package can override them.

## Consequences

- Ubuntu 24.04 gets an AppImage in v1.x rather than a native package.
- Flatpak and Snap are out of v1 because the sandbox blocks the virtual keyboard protocol, uinput and the Unix socket contract.
- Models never ship in a package; the catalogue downloads and verifies them.
- `docs/RELEASING.md` names the gate: full QA pack on the development machine, CI green, contract fixtures green, changelog and docs updated.
