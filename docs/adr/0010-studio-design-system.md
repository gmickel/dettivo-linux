# 0010. The Studio design system resolves every visual value from the Omarchy theme

Status: Accepted 2026-09-03

## What this gives you

Dettivo looks like part of Omarchy on every theme you switch to, within 100 ms of `omarchy theme set`, and the same screens read as a calm native app on GNOME or KDE. The OSD, the bar panel and the app share one set of components, so a fix lands everywhere at once.

## Situation

Omarchy's shell reads `~/.local/state/omarchy/current/theme/colors.toml` and `shell.toml` into `Color` and `Style` singletons: a sixteen-colour palette, per-state fill and border alphas for controls, a `monospace` font alias, base size 12, spacing tokens and a corner radius. Two directions were sketched, a dense terminal-monitor layout and a transcript-first editorial layout; the approved direction, Studio, keeps Dettivo's structure on Omarchy's material. Quickshell exposes no accessibility tree, and the plugin has to be QML.

## Decision

Design tokens come only from the theme files when present and from a built-in dark and light palette otherwise, following the portal colour scheme. The `Dettivo` QML module at `/usr/lib/dettivo/qml` carries the custom Quick Controls style, the type scale, spacing, the motion library, the icon set and every component. The app and `dettivo-osd` load it; the Omarchy plugin imports it by path and hosts the OSD as a shell panel, so no separate OSD process runs on Omarchy. Approved baselines under `docs/design/studio/baselines/` are the reference for every surface, and no screen ships without one.

## Consequences

- Monospace UI on Omarchy through the alias, system UI font plus monospace transcripts elsewhere.
- Corner radius follows the shell token; Black Gold renders square, other themes may round.
- Themes where accent equals foreground (Kanagawa) rely on the alpha fills for focus and selection, as the shell itself does.
- `colors.toml` alone is a theme. Omarchy generates `shell.toml` from its template only at `omarchy theme set`, older releases never wrote one, and every official theme ships without it, so requiring both files sent every such theme to the built-in palette. A shell token the theme leaves out takes the theme's own role (bar from the background, pressed and selection from the accent), never a literal colour of the built-in palette, and the `theme` block of `dettivo app status` and `dettivo osd status` names the source, the directory, the background, the accent and any parse error.
- The plugin checks the daemon's version on load and shows an install or upgrade hint instead of failing when the module or daemon is missing.
- Visual regression compares implementation against the baselines; a deviation is fixed or re-approved.
