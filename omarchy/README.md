# Dettivo for the Omarchy bar

Dictation state in the bar, the Dettivo panel on click, and the recording pill drawn by the shell. The widget shows idle, listening, transcribing, a meeting with its timer and an error from the daemon's event stream; the panel offers Dictate, the meeting action, the mode, which engine, language model and insertion target are in play, your last dictations and Open Dettivo.

## Install

```
yay -S dettivo-bin
dettivo setup omarchy
```

`dettivo setup omarchy` enables the daemon's socket, writes the Hyprland bindings, copies this plugin into `~/.config/omarchy/plugins/gmickel.dettivo/`, enables it in the bar's right section and reloads Hyprland and the shell. To take the plugin from this repository instead, so `omarchy plugin update` keeps it current:

```
omarchy plugin add https://github.com/gmickel/omarchy-dettivo.git --enable
```

The plugin needs `dettivo` on `PATH` and the shared QML module the package installs at `/usr/lib/dettivo/qml`; without them the bar shows a dimmed mark and the panel says what to install, and a daemon older than `omarchy.minDettivo` in `manifest.json` gets an upgrade hint.

## Settings

The three settings in the shell's sheet (`glyph`, `levelMeter`, `osd`) are written to `[omarchy]` in `~/.config/dettivo/config.toml` through `dettivo config set`; the file is the source of truth and this folder carries no state. `osd = "panel"` (the default) makes this plugin host the recording pill and `dettivo-osd` steps aside; `service` leaves the pill to `dettivo-osd.service`; `off` shows none.

## Where it comes from

This folder is authored, linted, rendered and tested in the Dettivo for Linux repository (`omarchy/` there); the `omarchy-dettivo` repository mirrors it byte for byte and is written by the release, never by hand. Report a problem or send a change to the main repository.

## Licence

GPL-3.0-or-later, see `LICENSE`.
