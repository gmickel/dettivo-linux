# Dettivo for the Omarchy bar

Local dictation and meeting transcription, right in your Omarchy bar. The mark in the bar tells you what Dettivo is doing: idle, listening, transcribing, or recording a meeting with its timer. Click it and the panel lets you dictate, start or stop a meeting, switch between Raw, Polish and Enhanced, and see your last few dictations. The recording pill is drawn by the shell itself, and every colour and font comes from your current Omarchy theme.

![The Dettivo mark in the Omarchy bar with its panel open during a meeting](https://raw.githubusercontent.com/gmickel/dettivo-linux/main/docs/images/omarchy-bar-panel.png)

This plugin is the bar side of [Dettivo for Linux](https://github.com/gmickel/dettivo-linux), a speech workstation that runs Whisper, Parakeet and a local language model on your own machine. Hold `F9`, talk, let go, and the words land in the window you were typing in. Nothing leaves the machine unless you point Dettivo at an endpoint yourself.

## Install

Install Dettivo and let it set up Omarchy for you:

```bash
sudo pacman -U "$(curl -s https://api.github.com/repos/gmickel/dettivo-linux/releases/latest | grep -o 'https://[^"]*dettivo-bin-[^"]*x86_64\.pkg\.tar\.zst' | head -1)"
dettivo setup omarchy
```

The package comes from the latest [Dettivo release](https://github.com/gmickel/dettivo-linux/releases). `yay -S dettivo-bin` replaces this once the AUR packages are published.

`dettivo setup omarchy` enables the daemon, writes the Hyprland bindings (the same `F9` and `Super+Ctrl+X` chords Omarchy uses for dictation), installs this plugin in the bar's right section and reloads the shell.

If you'd rather track this repository with Omarchy's plugin manager, so that `omarchy plugin update` keeps it current:

```bash
omarchy plugin add https://github.com/gmickel/omarchy-dettivo.git --enable
```

The plugin still needs Dettivo itself installed. Without it the bar shows a dimmed mark and the panel tells you what's missing. If your Dettivo is older than the plugin expects, you get an upgrade hint.

## Settings

Three settings show up in the shell's plugin sheet:

| Setting | Options |
|---|---|
| Bar glyph | The six-bar waveform or a single dot |
| Level meter | Bars follow your microphone while you talk, or stay still |
| Recording pill | `panel` (this plugin draws it, the default), `service` (Dettivo's own overlay), or `off` |

They're stored under `[omarchy]` in `~/.config/dettivo/config.toml`, so you can set them there too. The plugin keeps no state of its own.

## Issues and changes

This repository is a mirror. The plugin is developed, tested and released from the [`omarchy/` folder of Dettivo for Linux](https://github.com/gmickel/dettivo-linux/tree/main/omarchy), and every release copies it here unchanged. Please open [issues](https://github.com/gmickel/dettivo-linux/issues) and pull requests there.

## Licence

GPL-3.0-or-later, see `LICENSE`.
