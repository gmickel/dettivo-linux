---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-10-text-insertion-virtual-keyboard-libei.1 Implement fn-10: text insertion chain, target guards and the insertion matrix

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Text insertion lands where the user was typing. `dettivo-insert` now carries the whole backend chain from ADR 0007 as one unit: an in-process Wayland virtual keyboard that uploads a keymap generated for exactly the characters of the text, libei through the RemoteDesktop portal, `ydotool`, `xdotool`, clipboard plus an app-aware paste keystroke, and clipboard only. Focus probes (Hyprland IPC, X11 EWMH, a QA mock) verify the target before anything runs, the guards answer `CONFLICT`, a Dettivo window is refused with `target_is_self`, an unverifiable target only ever gets clipboard-only, and `insert.undo` takes a virtual-keyboard insertion back within the configured window. The daemon answers `insert.perform`, `insert.undo` and `insert.target`; the CLI gains `dettivo insert`, `insert undo`, `insert target` and backend lines in `doctor`; the QA rig gains the `insertion_matrix` and `never_into_self` drives; `[insert]` is a full `config.toml` section; `docs/insertion.md` explains it value first.

Commit: 9844897d73b866bede3cbebc5c05637b52df2243 on `fn-10-text-insertion-virtual-keyboard-libei` (worktree `~/work/dettivo-linux-wt/fn-10`, on top of the fn-4 QA rig branch). Gate `make build test lint` green, plus `cargo run -p xtask -- lint-deltas` and `dettivo-qa contract` (30 passed, the five insert fixtures among them).

### Requirement coverage

- R1 keymap generation: `crates/dettivo-insert/src/keymap.rs` tests `every_character_maps_to_exactly_one_keycode_and_compiles` (ASCII, accented Latin, CJK, emoji, mixed, each compiled with xkbcommon and typed back), `batches_split_above_the_keycode_budget_and_keep_order`, `unmappable_code_points_are_reported_by_index`, `named_keymaps_carry_modifier_and_editing_keys_on_their_physical_codes`, `generated_keys_avoid_physical_editing_and_modifier_codes`. Demotion for an unmappable text: `chain::tests::keymap_coverage_demotes_a_backend_for_that_text`.
- R2 chain selection: `chain.rs` tests `the_order_is_fr_i1_with_unavailable_backends_skipped_and_named`, `a_pin_selects_one_backend_and_an_unavailable_pin_fails_with_its_reason`, `an_unverifiable_target_permits_clipboard_only_and_clipboard_only_mode_too`; `backend::tests::the_chain_is_in_fr_i1_order`; live daemon `crates/dettivod/tests/insert.rs::a_pinned_backend_that_is_unavailable_fails_with_its_reason`.
- R3 target guards: `chain::tests::guards_mismatch_is_a_conflict_before_any_backend_runs`; live daemon `insert.rs::guard_mismatch_is_conflict_and_nothing_is_written` and `a_dettivo_window_as_target_fails_with_target_is_self_and_nothing_is_written`; the never-into-self drive `dettivo-qa drive never_into_self` passes on this Hyprland desktop (the QA target's app id joins `insert.self_app_ids` through `config.set`, `insert.perform` answers `failed`/`target_is_self`, the field stays empty).
- R4 real insertion: `dettivo-qa drive insertion_matrix --driver atspi` on this machine: `foot`, `ghostty` and `alacritty` receive `Dettivo café 日本語 42` through `virtual_keyboard` (116 to 122 ms), the Qt insert target receives it through `xdotool` (about 50 ms) because it is an XWayland window (see decisions), the result names backend and target, unavailable backends are logged with their reason (`backend skipped backend=libei reason=...`). CI runs both drives under Xvfb with `xdotool` added to the drive job. Clipboard restore is unit-tested with a fake clipboard (`backend::clipboard::tests`).
- R5 contract: `crates/dettivod/tests/contract.rs::implemented_fixtures_pass_against_the_live_socket` replays `insert/perform.json`, `perform.error-conflict-target-mismatch.json`, `perform.error-not-found-source-ref.json`, `target.json`, `undo.json` against a daemon in QA mock mode; `dettivo doctor` prints one line per backend (`cli.rs::doctor_reports_facts_and_exits_by_health`); `system.capabilities.platform.insertion_backend` names the chain's choice (`insert.rs::without_a_display_nothing_is_typed_and_a_guard_cannot_be_verified`, `platform.rs` tests); an unknown `source_ref` is `NOT_FOUND` (same test and the fixture). CLI end to end: `cli.rs::insert_commands_reach_the_daemon_and_validate_their_flags`.
- R6 matrix and config: `insertion-matrix.json` rows carry `target`, `outcome` (`pass`/`fail`/`skipped`), `backend`, `latency_ms`, `reason` (`scenarios/support.rs::rows_compare_the_read_back_with_the_sample`); kitty and chromium are recorded as `skipped` with the reason on this machine; the `[insert]` keys are in `DEFAULT_TOML`, `docs/config.md` and the regenerated `config/print_default.json` fixture (`schema.rs` tests, `edit.rs::lists_and_tables_coerce_from_json_and_from_typed_text`).

### Decisions

- XWayland windows never see a virtual keyboard's keymap on Hyprland (verified with `wtype` too: nothing arrives). The probe reports `xwayland`, the chain demotes the virtual keyboard for such targets with a logged reason and types through `xdotool` (paste keystrokes likewise); ADR 0007 gained that consequence.
- Generated keymaps allocate the 48 printable physical evdev codes first and leave modifier, editing, navigation, function and keypad codes out: ghostty interprets physical Backspace/Tab codes regardless of keysym, which cost characters until this change. Budget per batch is 160 distinct characters.
- `xdotool` gets at least a 40 ms inter-key delay for text outside ASCII (it remaps a spare keycode per character and Qt picks the mapping up asynchronously; 2 ms dropped characters).
- `insert.target` carries the backend availability list and the chain's choice, so `dettivo doctor` reads it from the daemon instead of a second contract addition.
- `DETTIVO_MOCK_INSERT=1` also mocks the focus probe (`org.gnome.TextEditor`, pid 4141) so the fixtures replay anywhere; `DETTIVO_MOCK_A11Y=1` mocks the probe alone. Two mock backends (`mock`, `mock_clipboard`) cover keystroke and clipboard-only modes.
- `insert.undo` after a non-undo backend answers `unsupported_backend` (the record is kept for every insertion), so the fixture order on one daemon is stable; `nothing_to_undo`, `expired`, `target_changed`, `backend_failed` are the other reasons.
- Config edits gained `List` and `Table` kinds (`foot, kitty` and `foot=ctrl+shift+v` coerce from typed text) so every `[insert]` key is editable through `dettivo config set`.
- Every insertion result carries a `context_pack` with `status = Off` and reason `context_capture_off`; the `perform.json` fixture was updated to that shape and the register documents it.
- Test daemons run without `WAYLAND_DISPLAY`/`DISPLAY`/`HYPRLAND_INSTANCE_SIGNATURE` so a unit test can never type into the desktop that runs it.
- libei is implemented (ashpd RemoteDesktop session, reis `ei_text` with a keycode fallback on the server keymap) but could not be exercised here: xdg-desktop-portal-hyprland has no RemoteDesktop interface, which the availability check reports. The `ei_text` path and the portal flow are untested against a live portal.
- The atspi driver now reads text with `GetText(0, -1)`: Qt's bridge answers that but not the `CharacterCount` property, which is why read-back returned the accessible name before.

### Notes for the conductor

- Secure-field detection over AT-SPI (FR-I6) is not implemented: `Target.secure_field` is always `false` and the chain rule (clipboard-only with reason `secure_field`) is in place for when a probe sets it. Follow-up.
- Foreign-toplevel (other wlroots compositors) and KDE window-management probes are not implemented; those sessions fall to the X11 probe under XWayland or to clipboard-only. Follow-up.
- The Chromium row of the matrix is always `skipped` (no read-back path without an agent-browser drive). Follow-up.
- On a live desktop the drives depend on the compositor honouring `focuswindow`; one run in three saw Hyprland keep focus elsewhere and the guards refused the insertion (recorded as a failure of that run, nothing typed elsewhere). CI under Xvfb focuses through the driver's click.
- `.github/actions/setup-toolchain/action.yml` adds `libxkbcommon wayland`; the drive job adds `xdotool` and the two new drives.
- `dettivo-insert` depends on `dettivo-proto` only; new workspace crates: wayland-client, wayland-protocols-misc, xkbcommon, wl-clipboard-rs, x11-clipboard, ashpd, reis, futures-util, enumflags2.

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: d00702f8040bfd8beb57454824f03a8a0e23cffe
- Tests: baseline: green (cargo build --workspace --all-targets && cargo test --workspace, pre-edit), make build test lint (rc=0), cargo run -q -p xtask -- lint-deltas (rc=0), cargo run -q -p dettivo-qa -- contract (30 passed, 0 failed, 1 skipped; insert/* fixtures pass), cargo run -q -p dettivo-qa -- lint-scenarios (4 scenarios use the driver interface only), cargo test -p dettivo-insert (26 passed: keymap, chain, guards, clipboard restore, probes), cargo test -p dettivod --test insert --test contract --test logs --test qa_env, cargo test -p dettivo-cli (insert_commands_reach_the_daemon_and_validate_their_flags, doctor backend lines), cargo run -q -p dettivo-qa -- drive insertion_matrix --driver atspi (pass on Hyprland: foot/ghostty/alacritty via virtual_keyboard, dettivo-insert-target via xdotool; kitty and chromium skipped with reason), cargo run -q -p dettivo-qa -- drive never_into_self --driver atspi (pass on Hyprland), make build test lint (exit 0) again after the rebase onto the audio layer via the rebased fn-4 branch
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
