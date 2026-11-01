---
title: "Sway repeats a held bindsym, which the hold key reads as a lost release"
date: "2026-11-01"
track: bug
category: integration
module: crates/dettivo-hotkeys/src/snippet.rs
tags: [hotkeys, sway, hold-key, desktop-entry]
problem_type: integration
symptoms: Holding the Sway hold chord ended dictation after about one second
root_cause: bindsym repeats while held; the press verb treats a press 1 s into a hold as a lost release
resolution_type: fix
---

## Problem
The hold key's state machine reads a press more than 1 s into a hold as a lost release and finishes the take. The generated Sway snippet bound the hold press with plain `bindsym`, and Sway repeats a held binding, so the first repeat after a second ended ordinary dictation before the key came up. A second review finding came from the same source-build path: the minimal `dettivo.desktop` used `Exec=dettivo-app`, which GLib rejects when the program is not on the portal's `PATH`, so the portal refused the app id.

## What Didn't Work
The original snippet relied on `start` being refused while a take ran, so repeats were harmless. That changed once the press verb gained a meaning for repeated presses after 1 s.

## Solution
`crates/dettivo-hotkeys/src/snippet.rs` binds the Sway hold press as `bindsym --no-repeat`. `crates/dettivo-hotkeys/src/identity.rs` writes the daemon's absolute path (`std::env::current_exe`), quoted per the Desktop Entry spec, as `Exec`. GLib accepts absolute and quoted absolute paths and rejects a bare missing name (checked through `Gio.DesktopAppInfo.new_from_filename`).

## Prevention
When a verb starts interpreting repeated or late events, audit every binding generator for compositor auto-repeat (`bindsym --no-repeat` on Sway; Hyprland's `bind` does not repeat unless `repeating`/`binde`). Any generated desktop entry needs an `Exec` that resolves without the user's PATH.
