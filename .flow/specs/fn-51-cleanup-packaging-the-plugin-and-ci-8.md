# Cleanup: packaging, the plugin and ci (8 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

The packages carry every library the binaries need, the plugin speaks the contract, and the workflows run as written. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0053 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### meetings

- **meetings/F19** (record): The decision record names migrations that do not exist
  - Where: `docs/adr/0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md:21` — “migration `0005-speakers`”; `docs/adr/0036-meeting-notes-analysis-search-export-and-the-delete-policy.md:17` — “Migration `0005-notes-analysis`”; `crates/dettivo-storage/src/migrate.rs:49` — `name: "0005-timings"`.
  - Why: The executable chain assigns speakers to migration 6 and notes/analysis to migration 7. ADR 0036 also describes six indexed columns, while the actual index has seven including speaker names. These discrepancies undermine the record used to assess upgrades.
  - Change: Add explicit corrections to the records and update the corresponding history documentation. Preserve the existing migration numbering.
  - Risk: Documentation-only. Check every migration reference against the registry and describe the complete FTS column set.

### qt-hosts

- **qt-hosts/F17** (contract): The declared Qt 6.8 minimum cannot compile the history filters
  - Where: `qt/CMakeLists.txt:29`, `find_package(Qt6 6.8 REQUIRED ...)`; `qt/host/app/history_model.cpp:408–409`, `beginFilterChange();` and `endFilterChange();`.
  - Why: These APIs were introduced in Qt 6.9 and 6.10 respectively. Configuration accepts a supported 6.8 installation that will then fail compilation. [Qt’s API documentation](https://doc.qt.io/qt-6/qsortfilterproxymodel.html#protected-functions) confirms the version requirements.
  - Change: Use the filter invalidation API available at the recorded minimum, or explicitly raise the minimum and update packaging and the ADR together.
  - Risk: Build against the minimum supported Qt as well as the current version; verify kind filtering and the newest-day proxy after updates.

### ops-and-record

- **ops-and-record/F2** (bug): Both packages omit a library the app requires
  - Where: `packaging/aur/dettivo-bin/PKGBUILD:17` and `packaging/aur/dettivo/PKGBUILD:20` list `'qt6-base' 'qt6-declarative' 'qt6-svg' 'qt6-wayland'`; `qt/host/app/CMakeLists.txt:68` links `Qt6::Multimedia`.
  - Why: Qt Multimedia is absent from both dependency lists. The existing release app’s ELF dependencies include `libQt6Multimedia.so.6`. CI’s build environment installs it separately, masking the source-package dependency omission.
  - Change: Declare Qt Multimedia and ensure a playback backend resolves through package dependencies. Regenerate both `.SRCINFO` files.
  - Risk: Backend selection affects playback. Build the source package with declared dependencies only, then install the binary package on a clean root and exercise playback.

- **ops-and-record/F3** (bug): The shared CI setup omits mandatory `jq`
  - Where: `.github/actions/setup-toolchain/action.yml:26` installs packages ending with `just zstd python`; `scripts/lint-omarchy-plugin.sh:28` exits with `"jq not found"`.
  - Why: The clean gate runs a mandatory lint whose executable is not provisioned. Settings and documentation checks also require `jq`.
  - Change: Add `jq` to the shared setup and the prerequisite check, so its absence is caught before compilation.
  - Risk: Minimal. Rehearse the gate in the pinned container without inherited host tools.

- **ops-and-record/F5** (contract): The plugin discards real dictation completions
  - Where: `omarchy/DettivoState.qml:185` handles `"completed"`; line 194 hides the pill on `"idle"`. `crates/dettivo-session/src/machine.rs:353` maps `Exit::Completed` to `State::Idle`. `qt/fixtures/omarchy-shell/tests/tst_plugin_events.qml:83` supplies `"state": "completed"`.
  - Why: The daemon attaches insertion results to the idle transition. The plugin therefore loses completion feedback and its history refresh. A probe using the actual completion shape produced `pillState: "hidden"`; the shim test passes because it invents the expected event.
  - Change: Consume idle events carrying completion data. Replace the invented event with a daemon-generated fixture and preserve failed-state feedback across the following idle transition.
  - Risk: Cancellation and ordinary idle events must still hide the pill. Cover insertion, clipboard fallback, failure, cancellation, and history refresh.

- **ops-and-record/F12** (bug): The plugin claims pill ownership without proving it can render
  - Where: `omarchy/Panel.qml:14` requires `moduleAvailable` for hosting; line 29 starts the ownership process using only `binaryAvailable` and panel mode.
  - Why: Ownership and rendering have different prerequisites. The plugin can suppress the standalone pill while its own module or loader is unavailable. Conversely, the window can render without confirmation that the ownership claim succeeded.
  - Change: Tie ownership to successful renderer readiness and observe the claim result. Release ownership when rendering fails.
  - Risk: Handover could briefly show two pills or none. Test missing modules, loader failure, competing ownership, and mode changes.

- **ops-and-record/F14** (contract): Documented QA commands do not match the recipes
  - Where: `docs/RELEASING.md:12` uses `just qa-release engines=<that directory>/debug`; `docs/guides/omarchy.md:37` uses `just qa-pack gui --surface omarchy`.
  - Why: The first passes the literal `engines=` prefix as part of the directory. A dry-run emitted `--engines engines=target/vulkan/debug`. The second failed with `justfile does not contain recipe '--surface'`.
  - Change: Document positional engine arguments and the existing `gui-omarchy` pack. Correct the same examples in recipe comments and other guides.
  - Risk: Minimal. Dry-run documented recipe invocations and check the resulting CLI arguments.

- **ops-and-record/F16** (contract): The crate-edge lint ignores native workspace dependencies
  - Where: `tools/xtask/src/edges.rs:262` filters dependencies with `n.starts_with("dettivo")`; lines 79–80 nevertheless list `parakeet-cpp-sys` and `sherpa-onnx-sys`.
  - Why: A client can acquire either native binding without this lint seeing the edge. The intended engine-process boundary is therefore unenforced precisely where linking native inference code matters.
  - Change: Identify internal dependencies through workspace membership, then declare the permitted native-binding edges.
  - Risk: Existing legitimate edges need explicit entries. Test metadata extraction with a client-to-native-binding dependency, not only manually constructed dependency lists.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** meetings/F19, The decision record names migrations that do not exist: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** qt-hosts/F17, The declared Qt 6.8 minimum cannot compile the history filters: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** ops-and-record/F2, Both packages omit a library the app requires: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** ops-and-record/F3, The shared CI setup omits mandatory `jq`: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** ops-and-record/F5, The plugin discards real dictation completions: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** ops-and-record/F12, The plugin claims pill ownership without proving it can render: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** ops-and-record/F14, Documented QA commands do not match the recipes: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R8:** ops-and-record/F16, The crate-edge lint ignores native workspace dependencies: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R9:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0053 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
