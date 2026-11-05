---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-82-hold-to-talk-that-survives-lost.1 Implement Hold-to-talk that survives lost releases, with a working portal identity

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Hold to talk now goes through one daemon state machine (repeat filter 1 s, double tap 300 ms, minimum hold 300 ms) fed by the new idempotent `dettivo dictation press`/`release` verbs, the portal and evdev, so a lost release finishes on the next press and a double tap locks recording (pill shows `locked · press to finish`). The portal backend registers the app id `dettivo` via `org.freedesktop.host.portal.Registry` before CreateSession (writing `dettivo.desktop` with the daemon's absolute Exec for source builds), shortcut ids carry the chord so rebinds take effect, and Hyprland's empty triggers report as `compositor_bound`; ADR 0083, docs/hotkeys.md, linux-deltas and CHANGELOG [Unreleased] cover it.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)
baseline: none recorded as red; Quick command `cargo test -p dettivo-hotkeys -p dettivod` green after the change (44 suites ok, 0 skipped); `just lint` green after commit.

R-ID tests: R1 crates/dettivo-hotkeys/tests/portal.rs (mock portal refuses an unregistered session) + identity.rs unit tests; R2 a_rebind_registers_the_new_chord_under_a_new_id_and_drops_the_old + daemon reload in crates/dettivod/tests/hotkeys.rs; R3 an_older_portal_and_hyprlands_empty_triggers_still_bind + output.rs hotkeys_lines test; R4/R5 crates/dettivo-hotkeys/src/hold.rs table tests + crates/dettivod/tests/hold.rs + the hotkeys_hyprland drive's lock and lost-release steps (live 3/3 after the QA fix below); R5 pill: osd_host_test.cpp and tst_plugin_hints.qml test_4; R6 crates/dettivod/tests/hold.rs + goldens; R7 docs.

Decisions: the Hyprland-submap Escape cancel and the keyless portal "Cancel dictation" shortcut from the spec's Approach (not in R1-R7) were left out and recorded in ADR 0083 as follow-ups: a submap left active after a take ends elsewhere disables every other Hyprland keybind, so it needs a daemon-side submap reset design. Qt sample snippet texts (first_run_model.cpp, hotkeys_setup_model.cpp) updated to the new verbs. The ADR was renumbered from 0081 to 0083 because fn-79 takes 0081 and 0082 on the goal branch.

QA fix (qa receipt NEEDS_WORK, S7): P1 S7-double-tap-drive fixed in 289cd05a7b1e5f84d30992ff335a1316f31f63ac. The hotkeys_hyprland double tap now keeps the modifiers down and taps only the hold key again, so the second press lands keys::SETTLE + TAP_GAP (140 ms) after the tap's release instead of about 380 ms. The test the_double_tap_lands_well_inside_the_daemons_300_ms_window bounds that gap; the product's 300 ms window is unchanged. P2 S7-release-lost: the daemon logs hold edges that did nothing at debug with the key's state (target dettivod::hold), and the drive enables that target. The live drive passed 3 of 3 runs (.flow/tmp/qa-fn-82-fix/drive-run{1,2,3}.log, evidence .flow/tmp/qa-fn-82-fix/qa-evidence/run-*). Run 3 reproduced the lost release (job_dict_1 duration_ms=11000); its daemon.log shows no hold edge between the press and the fixture's end, so that release never reached the daemon: Hyprland or the CLI dropped it, and the cause on that side stays open. The ADR renumber is 026a0bf34f41525566a8472bc3644a241dc35d56; the file rename itself landed in 289cd05.

Follow-ups: live R1 check on Omarchy (`dettivo doctor` identity line); P2 lost release on the Hyprland/CLI side (the release command never reached the daemon in 1 of 3 runs); feature map route change: the hold binding now runs `dettivo dictation press`/`release` (users re-run `dettivo setup` to pick it up; old start/stop bindings keep working).

stage: impl-review - ran (codex gpt-6.1-sol: 3-lens panel NEEDS_WORK with 3 findings fixed in 164ea9d, re-review SHIP)
stage: impl-review - ran (QA fix: codex gpt-6.1-sol high, one correctness reviewer over 8f885ae..026a0bf, SHIP, no findings)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: 642f8e5bb9a267e8f0d02610d16a40fed2d6d499, 164ea9d622d53761982dc47fc65c6e6b295dc87f, 289cd05a7b1e5f84d30992ff335a1316f31f63ac, 026a0bf34f41525566a8472bc3644a241dc35d56
- Tests: cargo test -p dettivo-hotkeys -p dettivod, cargo test -p dettivo-cli -p dettivo-proto -p dettivo-qa, just lint, ctest --test-dir build/qt -R dettivo-omarchy-plugin-test|dettivo-osd-host-test, cargo test -p dettivo-qa -p dettivod, dettivo-qa drive hotkeys_hyprland (3/3 pass)
- PRs: