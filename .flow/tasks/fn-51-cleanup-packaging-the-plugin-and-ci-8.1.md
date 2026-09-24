---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9]
---
# fn-51-cleanup-packaging-the-plugin-and-ci-8.1 Cleanup: packaging, the plugin and CI, every finding fixed or rejected with evidence

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-51 cleanup: packaging, the plugin and CI (8 review findings)

Every finding was verified against the code and a test before it was fixed; none was rejected. The gate `make build test lint` is green at d3d81b56 (rebased onto origin/main b79bba4, which gained nothing during the work), and both contract replays pass.

| Finding | Outcome | Commit / evidence |
|---|---|---|
| meetings/F19 (record names migrations that do not exist) | fixed | 110e1ce8. ADR 0035 said `0005-speakers`, ADR 0036 `0005-notes-analysis` and "six columns", docs/history.md the same name; `crates/dettivo-storage/src/migrate.rs` runs `0006-speakers` and `0007-notes-analysis`, and `0007-notes-analysis.sql` indexes seven columns (`speaker_names` included). Corrected in place; `scripts/check-docs.sh` check 10 fails on a backticked migration name that is not a file of `crates/dettivo-storage/migrations/`, and `scripts/test-check-docs.sh` plants one (it also caught ADR 0053's own quotation of the wrong names, d3d81b56). |
| qt-hosts/F17 (Qt 6.8 minimum cannot compile the history filters) | fixed | cf6f87e8. `qt/CMakeLists.txt` asks for Qt 6.8; `history_model.cpp` called `beginFilterChange()` (Qt 6.9 per the Qt docs) and `endFilterChange()` (6.10); the installed 6.11 header marks `invalidateRowsFilter()` deprecated only from 6.13. Both call sites now run the 6.10 pair behind `QT_VERSION_CHECK(6, 10, 0)` and `invalidateRowsFilter()` below. `scripts/lint-qt-floor.sh` (new, in `lint-toolchain`) named the four unguarded lines before the fix and passes after; the history filter tests in `dettivo-app-history-test` pass in the gate. |
| ops-and-record/F2 (both packages omit Qt Multimedia) | fixed | b805355f. `qt/host/app/CMakeLists.txt:70` links `Qt6::Multimedia`; neither PKGBUILD's `depends` had `qt6-multimedia` (`git show HEAD:... \| grep -c qt6-multimedia` = 0). Both recipes declare it, `.SRCINFO` regenerated with `makepkg --printsrcinfo`; `pacman -Si qt6-multimedia` shows it depends on the `qt6-multimedia-backend` provider, so a playback backend resolves through the package. `scripts/packaging/lint.sh` maps every `Qt6::<Module>` in the CMake lists to its package and named both recipes with the line reverted. README's toolchain line names qt6-multimedia and jq. |
| ops-and-record/F3 (CI setup omits jq) | fixed | cf6f87e8. `action.yml`'s pacman list ended `just zstd python`; the `archlinux:base-devel` image has no jq; `lint-omarchy-plugin.sh`, `lint-settings-keys.sh` and `check-docs.sh` need it. jq added to the action and to `scripts/check-toolchain.sh`; `scripts/test-check-toolchain.sh` (in `lint-toolchain`) failed with the action unchanged ("asks for jq ... but action.yml does not install it") and proves a hidden jq is named. |
| ops-and-record/F5 (plugin discards real completions) | fixed | dcb74ca5. `machine.rs::finish_shared` maps `Exit::Completed` to `State::Idle` with the insertion on that transition; the plugin handled `"completed"` and hid on `idle`, and read `target_app` as a string (it is `{bundle_id, name}`). Now `idle` from `inserting` is the completion (target name, history refresh), `idle` after `failed` keeps the error pill, cancel and its idle hide. New contract snapshot `crates/dettivo-proto/fixtures/events/dictation.state.event.json` (typed round trip in `fixtures.rs`), staged into `fixtures.js`; `tst_plugin_events.qml` feeds it. Against the old QML the test failed (`Actual idle, Expected inserted`). |
| ops-and-record/F12 (pill ownership without proof of rendering) | fixed | dcb74ca5. `Panel.qml` ran the claimer on `binaryAvailable && osdMode` while `hosting` also needed `moduleAvailable`. The claim now runs only while hosting is possible and the pill Loader has not errored; `claimed` follows the claimer's JSON answer and its exit; the window shows only while claimed. `tst_plugin_hints.qml` failed twice on the old QML (claiming true without the module; `claimed` undefined). |
| ops-and-record/F14 (documented QA commands do not match the recipes) | fixed | 110e1ce8. `just` treats `engines=<dir>` after a recipe as a positional argument and `--surface` as the next recipe (`just` is not installed here; the reviewer's dry run showed `--engines engines=target/vulkan/debug` and `justfile does not contain recipe '--surface'`). RELEASING.md, qa.md, guides/qa.md, guides/omarchy.md (`just qa-pack gui-omarchy`), polish-models.md and the justfile's recipe comments now pass positional arguments. `check-docs.sh` check 11 walks every `just` invocation in code spans and fences (unknown recipe, `name=value`, an extra argument that is not a recipe); before the doc fixes it named 15 lines; two plants in `test-check-docs.sh`. |
| ops-and-record/F16 (edge lint ignores native workspace dependencies) | fixed | cf6f87e8 (the `edges.rs` change was swept into that commit by `git add -A`; af15f317 carries the record and CONTRIBUTING). `member_from` filtered deps with `starts_with("dettivo")`, so `parakeet-cpp-sys` and `sherpa-onnx-sys` never appeared as edges. Membership now comes from `cargo metadata --no-deps`; the table declares `dettivo-engine-parakeet -> parakeet-cpp-sys` and `dettivo-engine-diarize -> sherpa-onnx-sys`; a binding on any other crate is named with the ADR 0003 reason. Test `metadata_keeps_native_workspace_edges` reads a client package with a binding dependency through the extractor and expects the violation (it would fail on the old filter, which drops the dep). `cargo run -p xtask -- lint-edges` exit 0. |

### ADR

`docs/adr/0053-packages-plugin-and-ci-carry-what-the-binaries-need.md` (Accepted 2026-09-06), indexed in `docs/adr/README.md`; amends 0013, 0030, 0034 and the edge rule of 0003/0019, corrects 0035 and 0036. Status lines of 0013, 0030, 0034 (amended) and 0035, 0036 (corrected) point at 0053.

### Verification

- `flock /tmp/dtv-gate.lock make build test lint` at d3d81b56: `GATE_EXIT=0` (log: scratchpad `fn51-gate.log`; the first run at af15f317 failed only on the new docs check naming ADR 0053's own quotations, fixed in d3d81b56).
- `cargo test -p xtask -p dettivo-proto`: exit 0. `cargo run -q -p xtask -- lint-edges`: exit 0.
- `cargo run -q -p dettivo-qa -- contract` and `-- contract --strict`: exit 0 (`rest: 55 passed, 0 failed`).
- `scripts/qa/omarchy-plugin-test.sh build/qt`: 9 passed after the fix; 3 failed against the old `omarchy/*.qml`.
- `scripts/lint-omarchy-plugin.sh build/qt`: OK. `scripts/test-check-docs.sh`: eight plants fail by file. `scripts/test-check-toolchain.sh`: ok. `scripts/packaging/lint.sh`: ok.
- `~/.local/share/dettivo/models`: 19 files before and after; no QA run here touches models (the plugin test runs under the shim, offline). My `models-before.txt` snapshot in the shared scratchpad was overwritten by a sibling worker's file at 17:05, so the byte comparison is by count and the after listing only.
- Qt build for the plugin tests ran directly (offscreen platform, no desktop); no drive was needed, so `xvfb-session.sh` was not used.

### Deliberately left out

- The Qt floor stays 6.8; raising it to 6.10 (the alternative the reviewer offered) would have changed ADR 0013 and the packaging for no gain on Omarchy's 6.11.
- No Loader-failure test for the panel: the shim cannot make `DettivoPill.qml` fail to load; the guard is in place and the window/claim tests cover the other prerequisites.
- The `just` arity check treats an argument past a recipe's parameters as the next recipe only when it is one; `*flags` tails are variadic and unchecked, as just treats them.
- Workflows stay disabled (Gordon's instruction); the CI change is proven by the self-test and the package list, not by a run.
## Evidence
- Commits: 110e1ce88de26052f96d6f57aa14f17fc6215272, cf6f87e863607d9b450c3895b6047c3699a6053c, b805355f4c7ac38653b66c81aa63cf9afb65addf, dcb74ca52379ef02554dd599bebc6b79165ae5ae, af15f317e149fe48cfd5a95af025bea6b2fd2f8c, d3d81b560753600f0995167f6e12133b81580149
- Tests: flock /tmp/dtv-gate.lock make build test lint, cargo test -p xtask -p dettivo-proto, cargo run -q -p xtask -- lint-edges, cargo run -q -p dettivo-qa -- contract, cargo run -q -p dettivo-qa -- contract --strict, scripts/qa/omarchy-plugin-test.sh build/qt, scripts/lint-omarchy-plugin.sh build/qt, scripts/lint-qt-floor.sh, scripts/test-check-toolchain.sh, scripts/test-check-docs.sh, scripts/packaging/lint.sh
- PRs:
stage: plan-sync - skipped(config: planSync.enabled != true)
stage: completion-review - skipped(config: review.backend=none)
