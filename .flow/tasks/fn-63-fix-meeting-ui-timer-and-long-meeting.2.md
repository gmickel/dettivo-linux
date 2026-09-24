# fn-63-fix-meeting-live-fragments-and-stop.2 Repair automatic meeting analysis and verify recognition/diarization findings

## Description
Diagnose and fix the automatic meeting analysis invalid_output failure with the selected local model; first reproduce in a safe isolated invocation. Verify an analysis of the retained dogfood transcript without overwriting the saved meeting until the implementation is tested. Keep raw or reversible meeting data out of ordinary repos, tests, logs, Flow and PRs. Private working area is /run/user/1000/dettivo-dogfood-113dc6c1 (0700). Check the recorded live/final differences and speaker assignment findings; add focused synthetic regression coverage for any concrete defect found. Do not invent participant count or claim diarization accuracy without a reference. Preserve existing saved audio/transcript. Fix established defects; explicitly report remaining accuracy questions.

**Touches:** crates/dettivo-language/**, crates/dettivo-engine-llm/**, crates/dettivod/src/llm_engine.rs, crates/dettivo-transcribe/**, crates/dettivo-meeting/**, crates/dettivo-engine-whisper/**, docs/**, relevant Rust tests. No Qt/Omarchy edits.

Acceptance: automatic analysis succeeds on a representative isolated case and the retained dogfood transcript or reports an exact external blocker; focused tests cover the discovered failure; transcript and speaker findings have evidence without overstating accuracy; saved meeting data remains intact. No full release.
## Acceptance
- Diagnose and correct the reproducible local analysis failure with focused regression coverage.
- Validate analysis of the retained meeting in an isolated working area; do not corrupt saved results.
- Evidence distinguishes recording integrity, speaker assignment coverage, and recognition accuracy; unverifiable human speaker identities are not fabricated.
- Relevant Rust tests and formatting pass; no full release.
## Done summary
Meeting analysis now recovers from transcript parts and summary groups that exceed the local model's context or output budget. The patched daemon analysed the retained meeting in an isolated shadow database in about 19 seconds, reaching ready with 8 calls and 6 parts under the existing 4096-token context and 1024-token output limit.

The provider preserves finish reasons and distinguishes context overflow from other failures. Truncated map results split to a bounded depth; summary merges retry smaller groups down to a pair. Decisions and actions are combined deterministically, preserving extracted facts. Cancelled generations follow the existing typed cancellation error path and never trigger split/retry. ADR 0062 and the meeting/config docs describe the behavior.

Focused regression tests failed before the fixes and passed afterwards. They cover truncation, context pressure, bounded map failure, oversized and truncated summary recovery, minimal-pair failure, exact fact union, cancellation mapping, and existing short-path behavior. Worker verification passed 72 language unit tests, 15 analysis golden cases, the daemon adapter regression, cargo fmt --check and git diff --check. The Fable bridge also passed affected daemon/session integration tests, clippy, rustdoc, file-length, dependency-edge and docs checks. Baseline was none because the task/spec defines no Quick commands. Gate classify required code gates; the full just build test lint remains conductor-owned per dispatch.

Original title, raw/final transcript, 948 segments, notes and both retained audio files were verified unchanged. Recording integrity and speaker-assignment preservation are supported by evidence. Remote speaker identities/count and recognition accuracy remain unverified; 154 remote segments remain unassigned, and the suspicious recognition span needs human listening. No speculative recognizer or diarization tuning was made. The shadow daemon was stopped after verification. Main service/config, package, push and release were untouched.

Provider-interface expansion required signature adaptations in Enhanced, pipeline and the session cancellation fixture; default non-local providers retain their existing rewrite behavior. No engine protocol/binary or Qt/Omarchy changes were needed.

stage: implement - ran (model: claude-fable-5-1; delegated: 0 across initial bridge and correction follow-up)
stage: impl-review - skipped(config: REVIEW_MODE=none, repository fast-build policy)

Conductor validation: complete just build test lint passed on the combined local hotfix (all Rust tests,81 Qt checks, lint and docs). The command used the installed Just1.58 binary and an isolated disk-backed /tmp mount because the host user quota was full; no tests or assertions were skipped. Native click/T rename, Enter save, Escape cancel and reopen persistence passed on the isolated meeting. Final default-size render confirms the status and footer fit. The local standalone UI bundle, daemon and plugin were installed and verified by hashes/process paths. Actual saved-meeting analysis reached ready with8 calls/6 parts under unchanged model limits. Original title/transcript/948 segments/notes/audio and config hashes remained unchanged. No full release; package remains0.1.0-1.58. Hosted CI workflows are manually disabled; this is local verification.

stage: full-gate - ran (just build test lint, disk-backed temp namespace)
stage: native-validation - ran (isolated and live local app)
stage: completion-review - skipped(config: repository fast-build policy)
stage: qa - skipped(config: pipeline.qa=off; native conductor checks recorded above)
stage: plan-sync - skipped(config: planSync.enabled=false)
## Evidence
- Commits: 73fa27c94903ff93a5798f1fdc6259ef9bd2e1b5, a2be6b5087f79e92de1db90fc84828f05a2cf103, d8bac4b6ed5144487a77584d9582c3a9b8bcfc47, 4b243167bf1b8dec918f5db4b77ff564da801304, 1aae6e3fbe928ccec795c4d8dec66fdd53d1985b
- Tests: baseline: none (task/spec Quick commands absent), cargo test -p dettivo-language -p dettivod --lib (72 language unit tests passed), cargo test -p dettivo-language --test analysis_goldens (15 cases passed), cargo test -p dettivod --bin dettivod llm_engine (passed), cargo fmt --check (passed), git diff --check (passed), gate classify: FULL, Rust code changed; full just build test lint assigned to conductor, isolated candidate-daemon retained-meeting analysis: ready in about19s, calls8 parts6; original saved-data/audio integrity unchanged, Fable bridge affected integration/clippy/rustdoc/file-length/edge/docs checks passed; private implementation-final-digest.md records details, Conductor full gate: just build test lint via isolated disk-backed /tmp; all Rust tests,81 Qt checks, lint/docs passed, Native title click/T, Enter/Escape and reopen persistence passed; status/footer visually checked, Installed local hotfix: actual saved-meeting analysis ready; config, title, transcript, segments, notes and both audio hashes unchanged
- PRs: