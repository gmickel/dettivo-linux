---
title: Hyprland discovery misses an absolute WAYLAND_DISPLAY; idle watch hides outages
date: "2026-11-02"
track: bug
category: runtime-errors
module: crates/dettivo-insert/src/instance.rs
tags: [hyprland, wayland, compositor-restart, fn-91]
problem_type: runtime-error
symptoms: "After a compositor restart with an absolute WAYLAND_DISPLAY, takes still go to the clipboard; a take after an outage is wrongly joined"
root_cause: "Display names compared as strings, and a reconnect treated as continuous observation"
resolution_type: fix
---

## Problem
The fn-91 discovery of the live Hyprland instance compared the `hyprland.lock` display line ("wayland-1") with `WAYLAND_DISPLAY` as strings. A session whose `WAYLAND_DISPLAY` is an absolute socket path (libwayland accepts one) never matched, so after a compositor restart every probe failed and takes went to the clipboard. Separately, the input-idle watch, once it reconnected, answered "no input" for spans that crossed the outage, so the next take could be joined to a previous one although the user typed meanwhile.

## What Didn't Work
String equality on display names, and a `watching: bool` flag that a reconnect sets back to true.

## Solution
Resolve both sides to socket paths before comparing: `Session::wayland_socket()` against `runtime_dir.join(lock_name)` (crates/dettivo-insert/src/instance.rs, `live_candidates`). The idle monitor keeps `watching_since: Option<Instant>` and answers `None` for any span starting before the current connection began (crates/dettivo-insert/src/idle.rs).

## Prevention
Any rule that matches a Wayland display must compare resolved socket paths and be tested with both a name and an absolute path. A watcher that reconnects must record when its continuous observation began; a reconnect is a gap, never a resume.
