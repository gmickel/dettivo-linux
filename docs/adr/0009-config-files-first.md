# 0009. config.toml is the source of truth and first run is three screens

Status: Accepted 2026-09-03, amended 2026-09-06 by 0049 (a broken file keeps the values in force and stops a start)

## What this gives you

You can set Dettivo up from a dotfiles repository and never see a wizard. When you do use the app, every settings route shows you the TOML it edits, and hand-written comments survive. A fresh machine needs three screens: keys, models, try it.

## Situation

Linux has no permission dialogs to walk a user through; the macOS onboarding exists mostly for microphone, input monitoring, accessibility and screen recording grants. Omarchy users manage configuration as files under `~/.config`. The Windows port kept a reconciling multi-step onboarding and stored settings as a JSON blob in SQLite, which hid the file from the user.

## Decision

`$XDG_CONFIG_HOME/dettivo/config.toml` holds every setting, documented key by key in `docs/config.md`. The daemon writes it with `toml_edit` so comments and ordering survive. `dettivo config get|set|unset|path|edit|validate` covers every key. Every settings route ends with a block that shows the keys it writes and a link to open the file. Runtime state that is not configuration (window geometry, last route, disclosure acknowledgement) lives in `$XDG_STATE_HOME/dettivo/state.toml`. First run shows three screens only when the daemon reports no speech model or no key bindings: Keys writes the compositor snippet and confirms live on key press, Models downloads a speech model and optionally the Enhanced model, Try it dictates into a test field. A provisioned config plus a populated model directory skips all three.

## Consequences

- The GUI validates through the daemon, so a hand edit and a settings change follow the same path.
- The meeting disclosure is a one-time dialog at the first meeting, never an onboarding step.
- Bindings live in a snippet the compositor includes (`~/.config/hypr/bindings/dettivo.conf` on Hyprland), so Dettivo never edits the user's main compositor config.
