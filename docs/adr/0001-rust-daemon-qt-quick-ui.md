# 0001. Rust behind the socket, Qt Quick for everything you see

Status: Accepted 2026-09-03

## What this gives you

The app, the OSD and the Omarchy bar panel look and behave like Omarchy's own apps, follow your theme, and share one set of components, while the daemon, engines, CLI and MCP server stay in one Rust workspace with one test suite.

## Situation

Omarchy 4 is a Qt desktop. The shell is Quickshell (Qt Quick), and Omawrite, Omacalc, Omacut, Omasnap and Omakade are all Qt 6. Omarchy sets `QT_QPA_PLATFORM=wayland;xcb` and ships `qt6-wayland` and `layer-shell-qt` by default. A libadwaita app would look like a guest and refuses theme overrides by design. The bar plugin has to be QML regardless of what the app uses. Qt 6 ships an AT-SPI bridge, so accessibility-tree automation keeps working.

For the Qt host (window, socket client, list models, theme watcher) the options were C++20, as the omacom apps do, or Rust through cxx-qt (0.10.0, August 2026, KDAB maintained). The host is view glue and stays under a few files either way; cxx-qt adds a code generator and a tokio-to-Qt bridge for no gain at that size.

## Decision

Rust for the daemon, engine processes, CLI, MCP server and QA runner. Qt 6 Quick for the app, the OSD and the Omarchy plugin, with QML in one shared `Dettivo` module installed at `/usr/lib/dettivo/qml` and a C++20 host per Qt binary. No third language and no web views.

## Consequences

- One repository with two build systems (Cargo and CMake) behind a single `justfile`; CI builds both.
- The app renders identically on GNOME, KDE and Hyprland because the custom Quick Controls style draws every control.
- cxx-qt remains the documented path if the host ever grows real logic (ADR 0001 would be superseded, the QML stays).
- QA drives address Qt widgets through AT-SPI, which requires `Accessible.name` on every control; a lint enforces it.
