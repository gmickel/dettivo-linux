---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-60-release-settings-and-reinsertion-ux.1 Implement release settings and reinsertion UX

## Description
TBD

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec criteria directly.

## Done summary
Implemented R1-R7 and delivered through merged PR #58, after prerequisite PR #57. The installed local package is dettivo-bin 0.1.0-1.58, built from b5e1ee4d7200. Its production sources match the merged tree; subsequent differences are test formatting and Flow records.

Settings defaults to Simple, with technical controls and previews behind Advanced. Language/model selectors preserve custom and inherited values. Vocabulary has Add/Remove controls, transforms have named switches, and Reset uses canonical defaults. Composite list writes remain locked through acknowledgement and refresh, with refusal/disconnection recovery. History copies the transcript with paste guidance. The package generates its commented config reference offline from the canonical Rust defaults.

### Verification

- Full workspace Rust tests and all 80 CTest cases passed in the isolated combined run (`/tmp/fn60-land-test-lint.log`). That aggregate stopped at formatting in two new QML tests. Formatting was corrected without changing assertions; the final `just lint`, including the Qt build, clippy, QML, accessibility, packaging and docs checks, passed (`/tmp/fn60-land-lint-final.log`, exit 0).
- An earlier aggregate hit the existing binary-fixture test once; its focused rerun and the full test rerun passed with the assertion unchanged (`/tmp/fn60-land-binary-test.log`).
- Focused regressions cover History copying, cold advanced deep links, custom choices, vocabulary terms containing commas, pending edits and failures, reset collisions and the Advanced button's checked binding. Bugbot's claimed binding loss did not reproduce; regression coverage was retained. The confirmed rapid-write defect was fixed.
- The package build initially exposed a daemon dependency in config-reference generation. The offline generator now runs with an unreachable daemon, parses as TOML and exactly matches the canonical protocol fixture (`/tmp/fn60-offline-config.toml`). The rebuilt archive and Arch package passed manifest and namcap checks (`/tmp/fn60-land-package-2.log`, `/tmp/fn60-land-namcap58.log`).
- Prefix installation has 16 passing checks, including installed Home rendering, engine executables, CPU/Vulkan Whisper and mock dictation at 285 ms. Fish and two optional model fixtures were absent, leaving three skips and a strict aggregate failure. This is not full release certification (`/tmp/fn60-land-package/install-test.json`).
- The real installation succeeded. Package integrity reports 273 files and zero altered files; running app/daemon hashes match the installed binaries. Configuration is unchanged. The native Settings UI renders correctly, the plugin matches the repository, all engines resolve from the installed directory, both F9 bindings remain present and Hyprland reports no configuration errors. Private verification receipt and screenshot are under `~/.local/state/dettivo-upgrades/20260915-fn60/`.

Bugbot reviewed both final patches with no new issues and zero open threads, per Gordon's explicit reviewer choice after Copilot reported its exhausted quota. Both patience windows elapsed. After PR #57 merged, the retargeted #58 tree and stable patch were identical to the tested, reviewed tree. Merge commits are 5ed6eb7d (#57) and 065ef944 (#58).

### Limits

Controlled native F9/panel acceptance remains pending under fn-59; no native meeting acceptance or visual baseline approval is claimed. No new public release/tag was published. The initial implementation baseline ran a live audio test before isolation was applied; it attempted restoration, and subsequent inspection found the normal hardware source and no test sink modules. All later automated verification used private XDG/D-Bus state and unreachable audio services.

stage: implement - ran
stage: impl-review - skipped(policy: project fast-build host reviews off)
stage: bugbot-review - ran (explicit user reviewer override)
stage: plan-sync - skipped(config: disabled)
stage: completion-review - skipped(policy: project fast-build reviews off)
stage: qa - skipped(config: pipeline.qa=off; isolated checks and native installation smoke recorded separately)
stage: land - ran
stage: install - ran
## Evidence
- Commits: 065ef944ba5b419746e13bb100946415a1d9f037
- Tests: baseline: red (just build test lint before implementation: missing evidence-map routes; /tmp/fn60-baseline.log), bash /tmp/fn60-isolated-gate.sh just test-rust test-diarization-score — passed; /tmp/fn60-full-tests.log, bash /tmp/fn60-isolated-gate.sh cargo test -p dettivod --test cleanup_dispatch — 3 passed; /tmp/fn60-cleanup-dispatch.log, ctest --test-dir build/qt --output-on-failure — 79/79 passed in isolated environment; /tmp/fn60-qt-lint-final.log, bash /tmp/fn60-isolated-gate.sh env CMAKE_BUILD_PARALLEL_LEVEL=8 just lint — passed, including final Qt build; /tmp/fn60-lint-final.log, History copy regression red-to-green; /tmp/fn60-history-red.log and /tmp/fn60-history-green.log, Cold advanced deep-link regression red-to-green; /tmp/fn60-deeplink-cold-red.log and final Qt suite, Isolated sample visual inspection: General, Hotkeys, Models choices, Polish, Meetings, Agents, Vocabulary, History and advanced deep link; no native acceptance or baseline approval claimed, git diff --check — passed, Full Rust workspace and 80/80 CTest tests passed; /tmp/fn60-land-test-lint.log (aggregate stopped at QML formatting), Final isolated just lint passed after formatting; /tmp/fn60-land-lint-final.log, Package and namcap passed; prefix install 16 pass / 3 missing-prerequisite skips, strict aggregate incomplete; /tmp/fn60-land-package/install-test.json, Installed 0.1.0-1.58: 273 package files, zero altered; native UI and running binary identities verified; config unchanged, Restacked tree and stable patch match Bugbot-reviewed c4615f24; merged production source matches installed b5e1ee4d
- PRs: https://github.com/gmickel/dettivo-linux/pull/58