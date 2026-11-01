---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-82-hold-to-talk-that-survives-lost.1 Implement Hold-to-talk that survives lost releases, with a working portal identity

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Hold to talk now goes through one daemon state machine (repeat filter 1 s, double tap 300 ms, minimum hold 300 ms) fed by the new idempotent `dettivo dictation press`/`release` verbs, the portal and evdev, so a lost release finishes on the next press and a double tap locks recording (pill shows `locked · press to finish`). The portal backend registers the app id `dettivo` via `org.freedesktop.host.portal.Registry` before CreateSession (writing `dettivo.desktop` with the daemon's absolute Exec for source builds), shortcut ids carry the chord so rebinds take effect, and Hyprland's empty triggers report as `compositor_bound`; ADR 0081, docs/hotkeys.md, linux-deltas and CHANGELOG [Unreleased] cover it.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)
baseline: none recorded as red; Quick command `cargo test -p dettivo-hotkeys -p dettivod` green after the change (44 suites ok, 0 skipped); `just lint` green after commit.

R-ID tests: R1 crates/dettivo-hotkeys/tests/portal.rs (mock portal refuses an unregistered session) + identity.rs unit tests; R2 a_rebind_registers_the_new_chord_under_a_new_id_and_drops_the_old + daemon reload in crates/dettivod/tests/hotkeys.rs; R3 an_older_portal_and_hyprlands_empty_triggers_still_bind + output.rs hotkeys_lines test; R4/R5 crates/dettivo-hotkeys/src/hold.rs table tests + crates/dettivod/tests/hold.rs + QA drive steps in hotkeys_hyprland (written, not run: live compositor QA is the conductor's); R5 pill: osd_host_test.cpp and tst_plugin_hints.qml test_4; R6 crates/dettivod/tests/hold.rs + goldens; R7 docs.

Decisions: the Hyprland-submap Escape cancel and the keyless portal "Cancel dictation" shortcut from the spec's Approach (not in R1-R7) were left out and recorded in ADR 0081 as follow-ups: a submap left active after a take ends elsewhere disables every other Hyprland keybind, so it needs a daemon-side submap reset design. Qt sample snippet texts (first_run_model.cpp, hotkeys_setup_model.cpp) updated to the new verbs. ADR number 0081 may collide with another fn-88 child at integration.

Follow-ups: live R1 check on Omarchy (`dettivo doctor` identity line) and `dettivo-qa drive hotkeys_hyprland` with the new lock and lost-release steps; feature map route change: the hold binding now runs `dettivo dictation press`/`release` (users re-run `dettivo setup` to pick it up; old start/stop bindings keep working).

stage: impl-review - ran (codex gpt-6.1-sol: 3-lens panel NEEDS_WORK with 3 findings fixed in 164ea9d, re-review SHIP)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 642f8e5bb9a267e8f0d02610d16a40fed2d6d499, 164ea9d622d53761982dc47fc65c6e6b295dc87f
- Tests: cargo test -p dettivo-hotkeys -p dettivod, cargo test -p dettivo-cli -p dettivo-proto -p dettivo-qa, just lint, ctest --test-dir build/qt -R dettivo-omarchy-plugin-test|dettivo-osd-host-test
- PRs: