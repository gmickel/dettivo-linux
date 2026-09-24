# fn-61-select-whisper-meeting-models.1 Fix independent meeting selection and run local dogfood hotfix

## Description
Whisper meeting models are selectable while Parakeet remains the dictation provider. The selector previously filtered out every Whisper entry under Parakeet. Explicit meeting selections now resolve Whisper in selection validation, reporting and meeting start; the new-meeting picker preserves dictation selection.

Verified focused Rust meeting and speech-model integration tests, including a new independent-provider meeting-start regression; all 80 Qt tests; dettivod clippy; formatting and diff checks. Visually verified the native settings screen with Parakeet dictation and Whisper Large v3 Turbo meeting selection. Live daemon subsequently reported recording_state=meeting during the user's dogfood test.

Installed a local hotfix only, as requested. The package remains dettivo-bin 0.1.0-1.58. Daemon binary is ~/.local/lib/dettivo-meeting-hotfix/dettivod, selected through ~/.config/systemd/user/dettivod.service.d/90-meeting-model-hotfix.conf. ~/.local/bin/dettivo-app launches the patched build/qt/apps/dettivo-app/dettivo-app; ~/.local/share/applications/dettivo.desktop selects that launcher. A later packaged delivery must retire these overrides after stopping the hotfix processes. Do not remove the build tree while this UI launcher depends on it.

No full rebuild, package release, push or merge. Source changes remain uncommitted. Normal integration and retirement of the local overrides remain downstream work.
## Acceptance
- [ ] TBD

## Done summary
Whisper meeting models are selectable while Parakeet remains the dictation provider. The selector previously filtered out every Whisper entry under Parakeet. Explicit meeting selections now resolve Whisper in selection validation, reporting and meeting start; the new-meeting picker preserves dictation selection.

Verified focused Rust meeting and speech-model integration tests, including a new independent-provider meeting-start regression; all 80 Qt tests; dettivod clippy; formatting and diff checks. Visually verified the native settings screen with Parakeet dictation and Whisper Large v3 Turbo meeting selection. Live daemon subsequently reported recording_state=meeting during the user's dogfood test.

Installed a local hotfix only, as requested. The package remains dettivo-bin 0.1.0-1.58. Daemon binary is ~/.local/lib/dettivo-meeting-hotfix/dettivod, selected through ~/.config/systemd/user/dettivod.service.d/90-meeting-model-hotfix.conf. ~/.local/bin/dettivo-app launches the patched build/qt/apps/dettivo-app/dettivo-app; ~/.local/share/applications/dettivo.desktop selects that launcher. A later packaged delivery must retire these overrides after stopping the hotfix processes. Do not remove the build tree while this UI launcher depends on it.

No full rebuild, package release, push or merge. Source changes remain uncommitted. Normal integration and retirement of the local overrides remain downstream work.

Conductor validation: complete just build test lint passed on the combined local hotfix (all Rust tests,81 Qt checks, lint and docs). The command used the installed Just1.58 binary and an isolated disk-backed /tmp mount because the host user quota was full; no tests or assertions were skipped. Native click/T rename, Enter save, Escape cancel and reopen persistence passed on the isolated meeting. Final default-size render confirms the status and footer fit. The local standalone UI bundle, daemon and plugin were installed and verified by hashes/process paths. Actual saved-meeting analysis reached ready with8 calls/6 parts under unchanged model limits. Original title/transcript/948 segments/notes/audio and config hashes remained unchanged. No full release; package remains0.1.0-1.58. Hosted CI workflows are manually disabled; this is local verification.

stage: full-gate - ran (just build test lint, disk-backed temp namespace)
stage: native-validation - ran (isolated and live local app)
stage: completion-review - skipped(config: repository fast-build policy)
stage: qa - skipped(config: pipeline.qa=off; native conductor checks recorded above)
stage: plan-sync - skipped(config: planSync.enabled=false)
## Evidence
- Commits: 3b805112
- Tests: cargo test -p dettivod --test speech_models --test meetings, cargo test -p dettivod --test meetings whisper_meetings_leave_parakeet_dictation_selected -- --nocapture, ctest --test-dir build/qt --output-on-failure (80 passed), cargo clippy -p dettivod --all-targets -- -D warnings, Native settings visual check; live user meeting recording_state=meeting, Conductor full gate: just build test lint via isolated disk-backed /tmp; all Rust tests,81 Qt checks, lint/docs passed, Native title click/T, Enter/Escape and reopen persistence passed; status/footer visually checked, Installed local hotfix: actual saved-meeting analysis ready; config, title, transcript, segments, notes and both audio hashes unchanged
- PRs: