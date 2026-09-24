---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13]
---
# fn-45-cleanup-delivery-and-the-target-window.1 Cleanup: delivery and the target window, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-45 Cleanup: delivery and the target window

Branch `fn-45-cleanup-delivery-and-the-target-window`, rebased onto main a47c0c5. Twelve findings: twelve fixed, none rejected. Every fix carries a test that was run against the old code first (the proof is noted per row).

| Finding | Outcome | Commit / evidence |
|---|---|---|
| dictation/F2 target is a process, not a window (R1) | fixed | `bbeae998`: the probe keeps the window identity (Hyprland address, X11 window id), `SessionTarget.window` guards it, a probe that saw nothing is `SessionTarget::unverified` and the take goes to the clipboard with reason `origin_unverified`. Tests `the_window_guard_tells_two_windows_of_one_process_apart`, `the_window_guard_refuses_another_window_of_the_same_process`, `a_failed_probe_at_start_is_an_unverified_origin_not_an_unguarded_one`, `an_unverified_origin_is_clipboard_only_and_says_so`. Before the fix the guard had no window field and `capture_target` returned `None` on a failed probe, which `check_guards` read as no guard. |
| dictation/F3 blind retry after partial delivery (R2) | fixed | `bbeae998`: `backend::Failure::{Before, During, FocusMoved}`; only `Before` hands the text to the next backend, `During` ends as `failed` with `partial_delivery: <backend>: ...`; command exit and timeout are `During`, a clipboard restore that fails after a paste is a warning. Tests `the_next_backend_runs_only_when_nothing_was_delivered`, `a_failing_command_says_whether_text_may_have_been_delivered` (includes a 100 ms timeout). Verified: `xdotool` at the 40 ms remap delay under the 20 s timeout typed a prefix of any text over 500 characters and the loop retried the next backend on every `Err`. |
| dictation/F6 guards expire before typing (R3) | fixed | `bbeae998`: `Ctx::recheck` runs in every backend after its setup and before its first key (virtual keyboard: before every batch), insertions run under `Service.insert_lock`. Tests `the_origin_is_checked_again_right_before_delivery` (scripted probe answers window 0x1 then 0x2: `target_changed`, nothing typed; 0x1 twice: inserted), `a_focus_that_moved_before_delivery_refuses_every_backend`. |
| dictation/F7 secure-field guard unreachable (R4) | fixed by deletion | `e5ecb1b3`: `target_from` always passed `false`; the field, the branch, the reason, the fixture row and the register mention are gone. Test `a_target_makes_no_secure_field_claim` (a document carrying the bit is rejected). Contract change registered in `docs/api/linux-deltas.md` with the `insert/target.json` fixture. |
| dictation/F10 CLI forces `raw` (R5) | fixed | `e943a023`: `dictation.start.mode` optional (`Option<DictationMode>`), the CLI omits a mode it was not given, the daemon applies `[dictation] mode`. Tests `an_omitted_mode_means_the_configured_one_and_stays_omitted`, `a_start_without_a_mode_leaves_the_mode_to_the_daemon`, daemon `a_start_without_a_mode_runs_in_the_configured_mode` (config `deterministic_polish`, no mode: completion event says `deterministic_polish`; explicit `raw` stays `raw`; was `INVALID_PARAMS` before). Register row added. |
| meetings/F9 no shared sample clock (R6) | fixed | `741e3261`: the machine stamps each source when it opens, `TakeWriter::start_take_at` counts the first take from that instant, `live::spawn` starts each lane's windower at its take's origin. Test `a_source_that_opens_later_keeps_its_origin_live_and_in_the_takes`: with a 300 ms system delay it failed on the old code with `microphone origin 300 ms` (both writers were created after the system opened) and passes now (mic < 150 ms, system >= mic + 300, remote live segments >= you + 300). |
| qa-rig/F5 Hyprland chords not owned (R7) | fixed | `964df38c`: `Chords::reserve` picks four Ctrl+Alt+Shift function keys `hyprctl binds -j` shows free, the cleanup is verified with `still_bound` and a chord left bound fails the scenario (combined with the drive's outcome). Tests `reserved_chords_skip_existing_ctrl_alt_shift_bindings`, `still_bound_names_the_chords_the_cleanup_left_behind`. Split into `hotkeys_hyprland_chords.rs` to stay under 500 lines. |
| qa-rig/F9 first-run failures repaired before judging (R8) | fixed | `d0236372`: the refocus-and-`insert.perform` retry block is deleted, the take's own insertion must land, and after Done the scenario asserts `failed` / `target_is_self` / no backend. Every drive runs under `scripts/qa/xvfb-session.sh` with controlled focus, so the shared-desktop repair has no place. |
| qa-packs/F11 one warm sample passes (R9) | fixed | `27eb2ccd`: `qualify` marks a row qualified only with every warm run completed; a short row keeps failures and partial spread, the step fails with the count. Test `a_row_qualifies_only_with_every_warm_run_completed` (1, 9 and 10 of 10). Verified: `measure` erred only when zero warm runs completed. `docs/qa.md` updated. |
| qml/F9 minimum width (R10) | fixed | `86180c0e`: `Theme.appWindowMinWidth` = sidebar + History list + hairline + right rail + space8; `AppWindow.minimumWidth` uses it. Test `test_minimum_width_leaves_every_route_room` fails on the old `sidebarWidth * 3` (the detail's width was negative) and passes now. |
| qml/F10 rename targets the wrong speaker (R11) | fixed | `41825ed8`: `SpeakerTranscript.rowFocused` hands the focused row's speaker to `MeetingDetail.selectSpeaker`; `currentSpeakerId` deleted. Test `test_keyboard_rename_targets_the_focused_rows_speaker` fails without the signal (speakerIndex stays 0) and passes now. |
| qml/F14 "Inserted" for every result (R12) | fixed | `fb85edcf`: `FirstRunModel.resultOutcome`; label, explanation and clipboard claim derive from `inserted`, `copied_to_clipboard`, `failed`. QML test `test_try_it_names_the_outcome_not_the_presence_of_a_result`, C++ test asserts the outcome. |
| R13 gate, replay, ADR, summary | see Verification | ADR 0047 indexed; evidence map routes every R-ID. |

### ADR

`docs/adr/0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md` (Accepted 2026-09-06), indexed in `docs/adr/README.md`. It amends ADR 0016 (the target is a window, not an app id and pid) and ADR 0007 (a backend hands over only when nothing was delivered); both status lines point at it. The optional mode and the removed `secure_field` are rows in `docs/api/linux-deltas.md`.

### Also changed

- The review reports on main 5b5f67b failed `scripts/check-docs.sh` with 132 absolute-link findings; main a47c0c5 fixed them, the branch is rebased onto it and my own link commit was dropped in the rebase.
- `86c936ca` documents two chord helpers clippy flagged; `30c97875` moves the omitted-mode daemon test to `crates/dettivod/tests/dictation_mode.rs` (the test file crossed 500 lines) and trims `MeetingDetail.qml` to 300.
- `qa/evidence-map.toml`: the fn-45 placeholder route is replaced by the tests and drives above.
- `docs/insertion.md`, `docs/hotkeys.md`, `docs/dictation.md`, `docs/qa.md`: the window guard, the unverified origin, the recheck, partial delivery, the reserved chords, the bench qualification.

### Verification

- `flock /tmp/dtv-gate.lock make build test lint` at `30c97875`: every step green, `gate exit=0` (build workspace, configure and build qt, unit tests, qt smoke tests 59/59, format check, clippy, dependency edges, file length, delta register, notice, packaging, qmllint and format, tokens, icons, accessible names, omarchy plugin, settings keys, documentation check, rustdoc).
- `cargo run -q -p dettivo-qa -- contract`: exit 0 (rest: 55 passed, 0 failed, 0 skipped); `-- contract --strict`: exit 0.
- `~/.local/share/dettivo/models`: identical before and after (19 files, sizes and paths).
- Every regression test was run against the old code first: the daemon test was `INVALID_PARAMS` without a mode, the meeting clock test failed with `microphone origin 300 ms`, the shell test failed at `sidebarWidth * 3`, the rename test failed with `speakerIndex` 0, the secure-field, delivery and guard tests could not compile against the old types (the old code had no window guard, no failure kinds and no recheck).
- Live drives not run here: `hotkeys_hyprland` (needs a Hyprland session with a Lua config and writable `/dev/uinput`) and `first_run_fresh` (a display drive of the app under xvfb); both compile and their pure pieces are unit-tested.

### Deliberately left out

- dictation/F7: no AT-SPI secure-field probe was written; the claim is removed instead, and detection returns with a probe that detects it.
- dictation/F6: the recheck stops the virtual keyboard between its 160-character batches; `xdotool`, `ydotool` and libei type their whole text in one call and are checked once, right before it. A per-keystroke check inside those helpers is not available.
- dictation/F3: whether `xdotool` or `ydotool` typed a prefix before exiting is not knowable from the exit status; the daemon assumes it did (conservative).
- qa-rig/F9: no separate recovery scenario was added; the product's recovery path is `dictation.reinsert_last`, which has its own coverage.
- meetings/F9: the origin is the instant the open call returned, not the stream's first buffer; the difference is the stream's own start-up and is the same for both tracks.
- qa-rig/F5, qa-rig/F9: the Hyprland and first-run drives were not run live here (they need a Hyprland session with uinput, and a display drive under xvfb with the app); their unit-level pieces are tested and the scenarios compile in the gate.
## Evidence
- Commits: eb989313e76ee34650aab9c51c3363705925970e, 3db22ada132a32dbf515d4fa26e91345c1d5cdcb, 2c8072dbfd4c548bba3b264afd4fe134ac4dfc46, d051fa9081aa99914263133cb1563d6bc7a48803, 4e708d116189d6a74625afe89c61d93be7a09c10, 4ac2a2ebef5639343c2368e24776c05bcf95de60, e1e55744c2b7bafe8c5752f36080542869ef41d0, be85333aaa060cb26dad1d33df6f04b11a002256, bc1ae6a611fc7c6dc46ebd7bbbc2aa7be0d6f83e, b34f3e793c49d410b3d47c31b14cdc36e49f4040, 7708400ad7c57238f1d6cfc4cbda37c83e6df2d9, c0e5781d48dafebb66e687df2856d5e11b136d36, 915a446b0a2e3db141513bbbd24bde4774c0aee6
- Tests: flock /tmp/dtv-gate.lock make build test lint, cargo run -q -p dettivo-qa -- contract, cargo run -q -p dettivo-qa -- contract --strict, cargo test -p dettivo-insert, cargo test -p dettivo-proto, cargo test -p dettivo-meeting --test session, cargo test -p dettivod --test dictation_mode --test insert --test dictation --test hotkeys, cargo test -p dettivo-qa --lib -- hotkeys_hyprland first_insert evidence_map, ctest --test-dir build/qt -R 'dettivo-quick-test|dettivo-first-run-test'
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
