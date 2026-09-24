# fn-63-fix-meeting-live-fragments-and-stop.1 Fix live fragments, post-processing visibility, and inline meeting renaming

## Description
Implement the parent spec's UI requirements: keep the complete provisional tail per source; acknowledge Stop immediately and show recording-stopped/transcribing state unambiguously; prominently display queued/running/done/failed/skipped post-processing with available progress; add accessible inline title renaming and a conflict-free discoverable keyboard shortcut. Keep transcript available during later processing. Handle out-of-order async completion/events and avoid presenting failed analysis as successful completion. Speaker pass completion was verified; stale running state is not a confirmed preexisting bug.

**Touches:** qt/**, crates/dettivo-proto/**, crates/dettivod/src/handlers/**, crates/dettivod/src/diarization.rs, crates/dettivod/src/analysis.rs, crates/dettivod/src/meeting_observer.rs, crates/dettivod/tests/**, docs/**. Expand only if necessary for these UI contracts and report why. Do not alter recognition/analysis prompting or Omarchy timer implementation in this task; those have separate owners.

Quick checks: build affected Qt targets; run dettivo-app-meetings-test and dettivo-quick-test plus affected smoke tests; relevant Rust tests if Rust changes; formatter and diff check. Root conductor runs full gate once after all implementation. No full release, package, push, or merge. Do not restart the running app/daemon; the conductor handles local hotfix replacement after tests.

Confidential meeting evidence stays under /run/user/1000/dettivo-dogfood-113dc6c1 and ~/.local/share/dettivo; never copy raw speech, notes, names, audio or reversible content into repo/tests/Flow. Synthetic regression fixtures only. Parent spec evidence-capture-only boundary applied before Gordon authorized implementation; current instruction authorizes fixes.
## Acceptance
- Parent spec UI criteria for provisional fragments, stop acknowledgement, processing visibility and title renaming are satisfied.
- Focused regression coverage protects multi-fragment updates and terminal/queued processing transitions.
- Native surface retains theme styling and keyboard accessibility.
- Relevant focused checks pass; no live session or saved meeting is mutated during implementation.
## Done summary
Meeting users keep every provisional fragment, see Stop acknowledged immediately, and can follow queued, running and terminal transcript/speaker/analysis stages. The persistent processing strip exposes unavailable passes, analysis failures and unassigned segment counts; clicking the title or pressing T opens inline renaming with Enter save and Escape cancel.

The daemon persists queued pass states before publishing completion. Migration0008 preserves v7 rows, notes, segments and speakers; restart reconciliation settles abandoned queued/running passes. Detail reads use generations and bind progress through meetings.status to prevent stale responses or another meeting's job from changing the view.

Baseline was green (2 focused Qt suites). Final focused Qt5/5, Rust storage/proto/meeting and daemon rename/notes/diarize tests, QML lint/format, file lengths, Rust format and docs checks passed. import_long initially failed because the host /tmp user quota was exhausted; the conductor's unchanged test passed inside an isolated disk-backed /tmp mount namespace. Full repository gate and native shadow/live QA remain conductor-owned; no runtime, config or saved meeting was changed by this worker.

Dependency expansion covered dettivo-storage (queued schema/rename persistence), dettivo-meeting and daemon archive/hooks (publish queued intent before completion), daemon main (diarization module split), and schema version fixture expectations. An authorized preexisting settings test formatting issue was corrected. Recognition/analysis prompting and Omarchy timer remain separate.

stage: implement - ran (model: claude-fable-5-1; delegated: 3 across initial bridge and follow-ups; final whitespace-only correction by worker)
stage: impl-review - skipped(config: REVIEW_MODE=none, repository fast-build policy)

Conductor validation: complete just build test lint passed on the combined local hotfix (all Rust tests,81 Qt checks, lint and docs). The command used the installed Just1.58 binary and an isolated disk-backed /tmp mount because the host user quota was full; no tests or assertions were skipped. Native click/T rename, Enter save, Escape cancel and reopen persistence passed on the isolated meeting. Final default-size render confirms the status and footer fit. The local standalone UI bundle, daemon and plugin were installed and verified by hashes/process paths. Actual saved-meeting analysis reached ready with8 calls/6 parts under unchanged model limits. Original title/transcript/948 segments/notes/audio and config hashes remained unchanged. No full release; package remains0.1.0-1.58. Hosted CI workflows are manually disabled; this is local verification.

stage: full-gate - ran (just build test lint, disk-backed temp namespace)
stage: native-validation - ran (isolated and live local app)
stage: completion-review - skipped(config: repository fast-build policy)
stage: qa - skipped(config: pipeline.qa=off; native conductor checks recorded above)
stage: plan-sync - skipped(config: planSync.enabled=false)
## Evidence
- Commits: ffd960248d51b6c97b70e5ad3fcd7b25e25b34c3, 521dbec28ed1ea816d3fd71b7ff74cb8c90f54f5, 9c0edd42631e5cdbb669ee629351723ea15a130f, b0b0aaace2bcb584c68e8b7af2fe6f6e87dfff04, 70aea55e199b1e970aaabaf7707f8722322be86a, a29a832444d93758a9ec8e02fcc2929b744e36b4, ef934b507990405307662670f36bb241eaa20895
- Tests: baseline: green - dettivo-quick-test and dettivo-app-meetings-test (2/2), cmake --build build/qt (bridge), ctest --test-dir build/qt -R 'dettivo-quick-test|dettivo-app-meetings-test|dettivo-app-meeting-detail-test|dettivo-app-host-test|dettivo-app-cleanup-test' --output-on-failure (5/5), cargo test -p dettivo-storage -p dettivo-proto -p dettivo-meeting, cargo test -p dettivod --test meetings_rename --test meetings_notes --test meetings_diarize, scripts/qml-lint.sh build/qt, cargo run -q -p xtask -- lint-file-length, cargo fmt --all --check, scripts/check-docs.sh, git diff --check cbfefee8, cargo clippy -p dettivod --tests -- -D warnings (bridge), bwrap --die-with-parent --bind / / --dev-bind /dev /dev --proc /proc --bind /home/gordon/.cache/dettivo-qa-tmp /tmp -- target/debug/deps/import_long-5efcb5752640f6ca --nocapture (conductor,1 passed,34.73s), Conductor full gate: just build test lint via isolated disk-backed /tmp; all Rust tests,81 Qt checks, lint/docs passed, Native title click/T, Enter/Escape and reopen persistence passed; status/footer visually checked, Installed local hotfix: actual saved-meeting analysis ready; config, title, transcript, segments, notes and both audio hashes unchanged
- PRs: