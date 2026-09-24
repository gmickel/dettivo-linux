# 0053. The packages, the plugin and CI carry what the binaries need: the Qt floor holds, the plugin speaks the contract, the lints see every edge

Status: Accepted 2026-09-06. Amends 0013 (the Qt floor is enforced in the source), 0030 (the panel-hosted pill's completion and ownership), 0034 (the package dependencies) and the edge rule of 0003 and 0019 (native bindings are edges); corrects the migration names in 0035 and 0036.

## What this gives you

`yay -S dettivo-bin` installs an app whose history detail can play a take, because the package names Qt Multimedia. A build against Qt 6.8, the floor ADR 0013 promised, compiles. The Omarchy bar's pill shows where your words went and refreshes its recent list on every real completion, keeps a failure's reason on screen, and never suppresses the standalone pill while it has nothing to draw with. The clean CI container can run every lint of the gate. The crate-edge lint sees the native inference bindings, so no client can link one unnoticed. Every `just` command the documentation shows runs as written, and every migration a record names is one the daemon runs.

## Situation

A cross-model review of main `28b4b7d` (ten gpt-6-astra reviews, one per area) named eight places where the packaging, the plugin, CI or the record did not match the code: ops-and-record F2, F3, F5, F12, F14 and F16, qt-hosts F17, meetings F19. Each was verified against the code with a test that failed before its fix; none was rejected.

- Both AUR recipes listed `qt6-base`, `qt6-declarative`, `qt6-svg` and `qt6-wayland` while `qt/host/app` links `Qt6::Multimedia` for playback (ADR 0025); CI's container installed `qt6-multimedia` separately and hid the omission.
- `qt/CMakeLists.txt` accepts Qt 6.8 and `qt/host/app/history_model.cpp` called `beginFilterChange()` (Qt 6.9) and `endFilterChange()` (Qt 6.10), so a supported install configured and then failed to compile.
- The shared CI setup installed no `jq`, which `scripts/lint-omarchy-plugin.sh`, `scripts/lint-settings-keys.sh` and `scripts/check-docs.sh` read their fixtures with.
- `omarchy/DettivoState.qml` waited for `state = "completed"`, a transition the daemon never sends: the completion rides on `idle` from `inserting` with `insertion` and `first_words` (ADR 0015, `docs/api/linux-deltas.md`), and `target_app` is an object. The shim test invented the event it expected.
- `omarchy/Panel.qml` claimed `dev.dettivo.OmarchyPanel` whenever the binary was present in panel mode, while its window needed the shared module too; the window showed before the claim had answered.
- `docs/RELEASING.md`, `docs/qa.md`, the guides and `docs/polish-models.md` wrote the engines directory as a named argument (engines=, before the value) and narrowed the GUI pack with a surface flag after the recipe; just passes a named argument to the recipe as the literal text and reads a flag past the recipe's parameters as the next recipe.
- `tools/xtask/src/edges.rs` kept only dependencies named `dettivo*`, so `parakeet-cpp-sys` and `sherpa-onnx-sys` were never edges and a client could link either unseen.
- ADR 0035 and ADR 0036 named the speakers and the notes migrations with the number 5, which belongs to the timings migration, and described a six-column index; the chain runs `0006-speakers` and `0007-notes-analysis` over seven columns.

## Decision

- **A package names every Qt module a shipped binary links.** Both PKGBUILDs depend on `qt6-multimedia`, whose dependency on the `qt6-multimedia-backend` provider brings a playback backend with it. `scripts/packaging/lint.sh` maps every `Qt6::<Module>` in the CMake lists to its Arch package and fails when either depends list lacks it; a module the table does not know is a finding of its own. This amends ADR 0034's dependency list.
- **The Qt floor is enforced in the source, not only at configure time.** A Qt API newer than 6.8 appears in C++ only inside `#if QT_VERSION >= QT_VERSION_CHECK(...)` naming at least its version, with the floor's call on the `#else` side; `scripts/lint-qt-floor.sh` (in `just lint` through `lint-toolchain`) fails otherwise. The history proxies run `beginFilterChange()` and `endFilterChange()` from 6.10 and `invalidateRowsFilter()` below. The floor stays 6.8 (ADR 0013).
- **The prerequisite check and the CI setup name the same tools.** `scripts/check-toolchain.sh` names `jq` before anything compiles; `.github/actions/setup-toolchain/action.yml` installs it; `scripts/test-check-toolchain.sh` fails when a one-package hint of the check is missing from the CI package list, and proves the check names a hidden `jq`.
- **The plugin consumes the contract's completion.** `DettivoState` handles `idle` from `inserting` as the completion (insertion outcome, the target's `name` or `bundle_id`, the first words, the history refresh), keeps the reason on the pill through the `idle` that follows `failed`, and hides on `cancelled` and the `idle` after it. The contract carries `crates/dettivo-proto/fixtures/events/dictation.state.event.json`, the completion transition typed and round-tripped like every other event snapshot; the staging script writes it into `fixtures.js` and the shim test feeds it as the daemon would. This amends ADR 0030's event mapping.
- **Ownership follows rendering.** The `panel` kind runs `dettivo osd host-panel` only while hosting is possible (panel mode, the shared module present) and the pill component has not failed to load; `claimed` follows the claimer's answer and its exit, and the window shows only while claimed. A shell without the module leaves the name free and `dettivo-osd` keeps the pill. This amends ADR 0030's hand-over.
- **Documentation runs as written.** `scripts/check-docs.sh` fails on a `just` invocation in code that names no recipe, passes a `name=value` argument, or passes an argument past the recipe's parameters that is not itself a recipe; and on a backticked migration name that is not a file of `crates/dettivo-storage/migrations/`. Every example passes positional arguments and names the `gui-omarchy` pack; the justfile's recipe comments say the same.
- **An edge is a workspace dependency.** The edge lint decides membership by `cargo metadata --no-deps`, not by name, and the table declares the two native edges: `dettivo-engine-parakeet -> parakeet-cpp-sys` and `dettivo-engine-diarize -> sherpa-onnx-sys`. Any other crate that acquires a binding is named with the reason that a native inference binding links into its own engine binary only (ADR 0003).
- **The records name the chain.** ADR 0035 and ADR 0036 and `docs/history.md` name `0006-speakers`, `0007-notes-analysis` and the seven indexed columns (`speaker_names` among them); the migration numbering is unchanged.

## Consequences

- A new Qt module in the CMake lists needs its package in both PKGBUILDs and, when unknown to the table, one line in `scripts/packaging/lint.sh`.
- A Qt API from 6.9 or later costs a version check and a floor-side call; the lint's table names the APIs and the versions it knows and grows with each use.
- A tool a lint needs is added to `scripts/check-toolchain.sh` and the CI setup together; the self-test refuses one without the other.
- The plugin's completion, failure and cancel paths are pinned to the contract's shapes through the shim test; the daemon-side proof of the completion transition stays in `crates/dettivod/tests/dictation.rs`.
- Between a mode change to `panel` and the claimer's answer the window stays hidden for the claim's round trip; a claim refused by another holder leaves `dettivo-osd`'s pill, not the plugin's.
- A documented `just` command with a `name=value` argument fails the docs check; recipes take positional arguments and their `*flags` tail.
- A crate gaining a workspace dependency, native or not, edits the edge table; the reason in the violation says which rule it crossed.
