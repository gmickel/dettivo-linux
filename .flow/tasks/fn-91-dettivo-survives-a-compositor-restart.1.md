---
satisfies: [R1, R2, R3, R4]
---
# fn-91-dettivo-survives-a-compositor-restart.1 Implement Dettivo survives a compositor restart

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Dettivo now keeps typing after Hyprland restarts. The focus probe treats HYPRLAND_INSTANCE_SIGNATURE as a hint. On every refused connection it uses the newest live instance whose hyprland.lock names the daemon's Wayland display socket. Instances of another display are never connected to (ADR 0099). The screen-lock watcher no longer stops when a retry hits a refused display socket. That watcher and the input-idle watch now reconnect with a 100 ms to 2 s backoff, and the idle watch never counts an outage as quiet input. The portal backend binds the shortcuts again when xdg-desktop-portal-hyprland comes back (ADR 0100). insert.target carries a compositor block, and dettivo doctor shows it as a "compositor" row naming the instance in use and a stale environment signature. docs/insertion.md, docs/hotkeys.md, docs/api/linux-deltas.md and CHANGELOG are updated.

Tests (each red before its fix):
- R1: crates/dettivod/tests/compositor_restart.rs the_next_insertion_after_a_compositor_restart_types_again. The stand-in Hyprland and Wayland compositor restart under a running daemon. The next insert.perform types through the pinned virtual_keyboard within 2 s, and the log names one switch. The red run reproduced Gordon's live error verbatim ("pinned backend virtual_keyboard is unavailable: target cannot be verified").
- R2: a_daemon_started_with_a_stale_signature_types_through_the_live_instance. An instance of another display gets 0 queries.
- R3: ready_screen the_lock_is_followed_again_after_a_compositor_restart; idle the_input_idle_watch_follows_a_restarted_compositor (outage span answers None); hotkeys tests/portal.rs a_restarted_portal_backend_gets_the_shortcuts_bound_again; instance only_an_instance_of_this_display_is_ever_tried (name and absolute WAYLAND_DISPLAY). The virtual keyboard and clipboard already connected per insertion, and the R1 test shows the keyboard working after the restart.
- R4: insert.target compositor block asserted in the daemon tests; doctor_report the_compositor_row_names_the_instance_and_a_stale_environment.

Baseline: green (cargo test -p dettivo-insert -p dettivo-hotkeys -p dettivod, exit 0, 49 binaries). Verify: the same crates plus dettivo-cli and dettivo-proto, exit 0, 73 binaries. After the review fixes: dettivo-insert plus compositor_restart green. clippy -D warnings, fmt, lint-edges, lint-file-length, lint-deltas, lint-notice, check-docs, packaging lint and rustdoc -D warnings all pass. The full `just build test lint` gate was not run locally; CI owns it. Every test ran inside scripts/qa/xvfb-session.sh with the session's Wayland, Hyprland, bus and PipeWire variables removed and a private XDG_RUNTIME_DIR. hyprctl was never run, and nothing touched the live compositor.

Decisions: a portal rebind gets 5 tries (250 ms doubling). A restart of the xdg-desktop-portal frontend is not followed. A Hyprland without the display line in hyprland.lock keeps the old behaviour.
Follow-ups: the virtual keyboard still connects with connect_to_env, which is fine on the same display but reports a refused socket as "Could not find wayland compositor".

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran [codex gpt-6.1-sol: NEEDS_WORK (absolute WAYLAND_DISPLAY match P1, idle outage gap P2), both fixed in 3b2b325, re-review SHIP]

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 4c7b3d57df426b00ebd7f9f9d191b107f97d1146, 299d8bdbaa9e20386b7ebfd108609e3bef4455c3, 3b2b32508e27eaf7d0e463a3b044c502571d7fc2
- Tests: cargo test -p dettivo-insert -p dettivo-hotkeys -p dettivod -p dettivo-cli -p dettivo-proto (isolated, xvfb-session.sh), cargo test -p dettivo-insert; cargo test -p dettivod --test compositor_restart (after review fixes), cargo clippy -D warnings; xtask lint-edges/lint-file-length/lint-deltas/lint-notice; scripts/check-docs.sh; rustdoc -D warnings
- PRs: