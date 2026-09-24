---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8]
---
# fn-46-cleanup-speech-and-text-quality-7.1 Cleanup: speech and text quality, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-46 Cleanup: speech and text quality, every finding fixed or rejected with evidence

Branch `fn-46-cleanup-speech-and-text-quality-7` at `697e9c5d`, rebased onto main `a47c0c5`. Seven findings, seven fixed, none rejected. Every fix carries a regression test that was run against the pre-fix code and failed there (the "before" evidence column).

| Finding | R-ID | Outcome | Commit | Evidence |
|---|---|---|---|---|
| engines/F4 whole-chunk RMS classifies audible speech as silence | R1 | fixed | `e34a7a0a` | `filters::is_near_silent` judges every 200 ms frame (`SILENCE_FRAME`). Before: `a_brief_utterance_in_a_long_silence_reaches_the_engine` failed (engine calls 0, expected 1) on a 1 s utterance in 300 s; the whole-chunk RMS of that fixture is 0.0058, under the 0.0065 floor. After: `one_second_of_speech_in_five_minutes_of_silence_is_not_silent`, the job test and the all-silent fixture (still skipped) pass. |
| engines/F5 seam deduplication deletes repetitions across gaps | R2 | fixed | `50fbf742` | `merger::merge` appends disjoint chunks untouched; seam candidates are words inside the overlap widened by `SEAM_TOLERANCE_MS` (1500). Before: two disjoint chunks each saying `yes` merged to `yes`; `yes` at 1 s and `yes` at 9 s inside an 8–10 s overlap merged to `yes`. After: `a_repetition_across_a_gap_or_outside_the_shared_audio_stays` and `a_duplicate_inside_the_shared_audio_is_dropped_once` pass; the three-chunk golden is unchanged. |
| engines/F12 streamed LLM text disagrees with the final answer when a stop string spans tokens | R3 | fixed | `6cb83c35` | New `stop::Streamed` holds back the longest suffix that is a proper prefix of a stop string. Before: the old loop over pieces `1 2 3 \n` with stop `3\n` streamed `123` and answered `12` (temporary test of the old loop, removed). After: `a_stop_string_that_spans_tokens_is_never_streamed`, `a_held_prefix_is_released_when_the_match_fails_or_at_the_end`, `holding_back_respects_character_boundaries` pass; `engine.rs` shrinks to 447 lines. |
| dictation/F12 Polish edits inside tokens Raw protected | R4 | fixed | `9c9a7f68` | `polish::rewrite` hides the raw layer's protected spans (`raw::hide`) before every rule and restores them before the post-processors. Before: `um run \`um  i think\` now` came back `Run \` I think\` now.`. After: `protected_tokens_are_not_prose` passes for AsDictated, VeryCasual and Formal; all goldens (61 unit, polish_fallback, post_processors, at_prefix, warp) unchanged. |
| dictation/F17 numeric guard accepts changed numbers | R5 | fixed | `12c06d7c` | `guard::numbers` compares each dictated number whole (decimal point and colon kept, thousands commas and trailing sentence marks dropped) against one output number, with multiplicity. Before: `42`→`420`, `1.5`→`15`, `3 … 3`→`3` all returned `None`. After: `a_changed_number_is_rejected_even_when_its_digits_survive` passes; `1000`→`1,000` still accepted. |
| dictation/F18 silence detection depends on the UI meter interval | R6 | fixed | `7e53568b` | The worker takes the peak from every PCM chunk (`peak_of`, meter scale); `Event::Level` only feeds the display. Before: a 20 ms voiced take with no level event was `silent` and never reached the engine. After: `silence_is_read_from_the_samples_not_the_meter` passes, including a level event over silent samples staying silent. |
| qt-hosts/F2 meeting notes unprotected before the daemon confirms the save | R7 | fixed | `cc26069d` | New shared `NotesSaver` (draft revision vs acknowledged revision, one save in flight per meeting, retry of a refused/unanswered draft, edit-during-save sent after the answer, draft parked by meeting id when the meeting is left and sent on reconnect or revisit, late answers matched by meeting and revision); the detail takes the row's notes only when nothing is dirty; `dettivo-app` flushes both editors and `DaemonClient::finish()` on `aboutToQuit`. Before: `notesStayDirtyUntilTheDaemonAnswers` failed on the old models (second flush after a refused save sent nothing: 1 call, expected 2). After: passes; all 59 Qt tests pass. |
| Gate, ADR, summary | R8 | see Verification | `d4db59f5`, `0e0141f4`, `697e9c5d` | ADR 0048 written and indexed; fn-46 mapped in `qa/evidence-map.toml`. The gate is green at the final commit. |

### ADR

`docs/adr/0048-speech-and-text-hold-under-test-silence-seams-stop-strings-tokens-numbers-and-notes.md`, indexed in `docs/adr/README.md`. It amends 0022 (near-silence guard per frame; seam rule inside shared audio), 0023 (Polish hides protected tokens; numeric guard compares numbers), 0026 (a stop prefix is never streamed) and 0038 (notes draft dirty until acknowledged); each of those records' status line points forward. User docs updated where behaviour changed: `docs/config.md`, `docs/history.md`, `docs/polish.md`, `docs/engines.md`, `docs/app.md`, `docs/dictation.md`.

### Verification

- Rebased onto main `5b5f67b` and then `a47c0c5` (the coordinator's evidence-map routes, spec link fixes and review-report link fixes). The one conflict, `qa/evidence-map.toml`, was resolved by keeping fn-46's own routes over the placeholder route; `697e9c5d` restores the `[[spec]]` header the merge dropped. The second rebase applied cleanly.
- `flock /tmp/dtv-gate.lock make build test lint` at `697e9c5d`, from `target/gate.sh` under the machine-wide lock, `GATE_EXIT=0`. Every recipe green: [toolchain] check toolchain, [cargo] build workspace, [cmake] configure qt, [cmake] build qt, [cargo] unit tests, [ctest] qt smoke tests, [rustfmt] format check, [clippy] clippy, [xtask] crate dependency edges, [xtask] file length, [xtask] contract delta register, [xtask] notice, [packaging] packaging lint, [qmllint] qml lint and format, [tokens] qml token lint, [icons] icon lint, [a11y] accessible name lint, [omarchy] omarchy plugin lint, [settings] settings key coverage, [docs] documentation check, [docs] documentation check proves itself, [rustdoc] rustdoc. Qt: `100% tests passed out of 59`. Every Rust test passes, including `evidence_map::tests::the_checked_in_map_covers_every_rid_of_every_spec` and the docs check.
- `cargo run -q -p dettivo-qa -- contract` and `-- contract --strict`: 55 passed, 0 failed, 0 skipped.
- Per-crate before/after runs recorded in the table; each temporary "before" test was deleted after its run.
- No drive, pack or visual run was needed (no QML or drive surface changed); `~/.local/share/dettivo/models` listing (sizes and paths) identical before and after.
- All files stay under the limits: `engine.rs` 447, `merger.rs` 361, `guard.rs` 432, `meeting_detail_model.cpp` 460, `meeting_live_model.cpp` 456, `app_meetings_test.cpp` 471, `notes_saver.cpp` 161.

### Deliberately left out

- Signs and locale decimal commas in the numeric guard: the regex never captured a sign and a comma is read as thousands grouping (the existing `1000`→`1,000` golden); recorded in ADR 0048 as the bound.
- The Polish pass protects tokens regardless of `[dictation] protect_tokens`; threading the flag into `EffectivePolicy` would change the policy hash and every caller, wider than the finding.
- Parked notes drafts live in app memory only; an app that exits while disconnected loses them and the state reads `Not saved`.
## Evidence
- Commits: e34a7a0ac931e497a97096fd83b4cc829ddf524c, 50fbf74231e14976260d7b7bbd3b2ad555b328cf, 6cb83c35262721a17fc0bd15cb11e5ac9b691b67, 9c9a7f68d119e79f76ccb0c0bb98373aef61f88e, 12c06d7caab1ea21b0ad71d3955c3f6df5545575, 7e53568b926996da188428a92c80718a8a61352d, cc26069d3b72b93d833cdd95932a75e13ac06b18, d4db59f576d3ef8f5b36728a5bbd56a4c98600ab, 0e0141f47bc1e66bfd17f9c704bd0968f072ef93, 697e9c5d8ae228cca36090f4d5a82362b525a88a
- Tests: flock /tmp/dtv-gate.lock make build test lint, flock /tmp/dtv-gate.lock make test-qt lint, cargo test -p dettivo-transcribe, cargo test -p dettivo-engine-llm --bin dettivo-engine-llm, cargo test -p dettivo-language, cargo test -p dettivo-session, cargo test -p dettivo-qa --lib the_checked_in_map, ctest --test-dir build/qt --output-on-failure, cargo run -q -p dettivo-qa -- contract, cargo run -q -p dettivo-qa -- contract --strict
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
