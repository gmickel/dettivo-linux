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


QA fix (qa-fn91-S2e-env-other-display, P2): the daemon now checks the environment's HYPRLAND_INSTANCE_SIGNATURE against the session's display, as it already did for discovered instances. When that instance's hyprland.lock names another Wayland display (WAYLAND_DISPLAY absolute or relative), the hint is ignored and discovery picks this display's live instance. A lock with no display line, as older Hyprland releases write, keeps the old behaviour; ADR 0099 now says so. The reproduction landed red first in 14a6b79: an_environment_naming_another_display_uses_this_displays_instance names a live instance on wl-9 in the environment while the daemon runs on wl-0, and failed on live.queries() > 0. The fix is 28ebe77, together with the unit test an_environment_instance_whose_lock_names_no_display_is_kept. Checks: cargo test -p dettivo-insert (exit 0), cargo test -p dettivod --test compositor_restart (4/4, exit 0), just lint (exit 0). The QA harness scenario s_r2_stale passed twice. In S2e the wl-9 instance got 0 queries and the wl-0 instance got 2, the compositor block read {signature: live, environment_stale: true}, "x" was typed, and no shim was called. Logs: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/S2-r2-env-other-live.fix-run1.log and .fix-run2.log.

stage: impl-review - ran [QA fix: codex gpt-6.1-sol at high, base 34dac2b, head 28ebe77: SHIP, 0 findings]


QA fix 2 (qa-fn91-S2f-env-other-display-reported-as-restart, P2): when the environment names a live instance of another display, the diagnostics now say so instead of blaming a restart. insert.target's compositor block gains environment_other_display (serde default false; existing fields unchanged). dettivo doctor reads "Hyprland instance live; the environment's other belongs to another display", and a real restart still reads "... is stale (the compositor restarted)". The switch log carries both signatures as ADR 0099 says: from= is now the environment's (or previous) instance before the other-display filter, so it reads from=other to=live rather than from="none". ADR 0099, docs/api/linux-deltas.md and docs/insertion.md are updated. Tests red first, then green in b391a46: instance an_environment_instance_of_another_display_is_reported_as_such; doctor_report the_compositor_row_names_the_instance_and_a_stale_environment (new other-display case); compositor_restart asserts environment_other_display and the from=/to= pair in its three switch tests. Checks: cargo test -p dettivo-insert -p dettivo-cli -p dettivo-proto (exit 0, 209 passed), cargo test -p dettivod --test compositor_restart (4/4), just lint (exit 0). Harness re-run on b391a46 (.flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/fix-b391a46/): S1a-h and S2a-f all PASS. S2e doctor: "compositor Hyprland instance live; the environment's other belongs to another display"; journal: "hyprland: switched to the live instance from=other to=live". S1 doctor: "compositor Hyprland instance second; the environment's first is stale (the compositor restarted)"; journal: "from=first to=second", then "from=second to=third".

stage: impl-review - ran [QA fix 2: codex gpt-6.1-sol at high, base cb10bdf, head b391a46: SHIP, 3/3 draws, 0 findings]

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 4c7b3d57df426b00ebd7f9f9d191b107f97d1146, 299d8bdbaa9e20386b7ebfd108609e3bef4455c3, 3b2b32508e27eaf7d0e463a3b044c502571d7fc2, 14a6b7994a38ebdc2f5ede6a692202d087d9efaf, 28ebe77b49cdf795d7220a83dfe60827da8a110c, b391a46d4616ed2b99f35964ab3b95d31c7b19d8
- Tests: cargo test -p dettivo-insert -p dettivo-hotkeys -p dettivod -p dettivo-cli -p dettivo-proto (isolated, xvfb-session.sh), cargo test -p dettivo-insert; cargo test -p dettivod --test compositor_restart (after review fixes), cargo clippy -D warnings; xtask lint-edges/lint-file-length/lint-deltas/lint-notice; scripts/check-docs.sh; rustdoc -D warnings, QA fix: cargo test -p dettivo-insert; cargo test -p dettivod --test compositor_restart (red at 14a6b79, green at 28ebe77); just lint, QA fix: .flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/harness/qa.py s_r2_stale, twice (S2e other=0, live=2), QA fix 2: cargo test -p dettivo-insert -p dettivo-cli -p dettivo-proto; cargo test -p dettivod --test compositor_restart (red before b391a46, green at b391a46); just lint, QA fix 2: harness qa.py s_r2_stale s_r1_restart at b391a46 (.flow/tmp/qa-fn-91-dettivo-survives-a-compositor-restart/fix-b391a46/run.out): all PASS
- PRs: