---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-90-the-speech-check-runs-during-the-take.1 Implement The speech check runs during the take

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The Silero speech check now runs during the take, through a patched whisper.cpp that carries the LSTM state across pieces. At the release only the last samples remain, and the verdict is the whole-take one to the millisecond of speech. Parakeet now warms up on 90 s of silence, so no take compiles its own Vulkan kernels after a driver or engine update. That compile was the cause the four unexplained History stalls fit.

R1: `crates/dettivo-engine-whisper/tests/vad_stream.rs` feeds every gate fixture in four piece patterns and asserts `speech_ms` equal to the whole take. A state-reset mutation fails it (0 against 7,600 ms). fn-79's gate tests and fixtures pass unmodified, and a cut take or a failed check falls back to the whole-take pass (`a_failed_check_leaves_the_release_to_judge_the_whole_take`).
R2 (measured through the daemon, 5 runs each, RTX 4090 Vulkan, warm shader cache, GPU shared with fn-89): release-to-text p50 after the change was 489 ms at 30 s, 583 ms at 60 s and 890 ms at 120 s; before, 570 ms (489 ms without two contended 15 s takes), 722 ms and 1,271 ms. Report: docs/reports/benchmarks/release-to-text-2026-11-02.md and .json.
R3: `a_check_slower_than_the_capture_drops_no_sample` (10 s take, 200 ms per check piece; every sample is in the take, in order, and the check saw them all).
R4 (measured): with `stt_idle_seconds = 61`, so the warm-up runs, five first takes of 3 s after an idle unload took 266-267 ms against a median of 386 ms for warm takes. On a current cache, fn-80's preload at the press had already removed the after-idle stall (before: 275-279 ms). Engine probes on an empty cache showed what is left: the first 45 s take stalled 2.5-2.9 s and the first 90 s take about 0.3 s after a 30 s warm-up, and nothing stalled after a 90 s warm-up.
R5: ADR 0097 (incremental gate and before/after numbers) and ADR 0098 (stall investigation, the options measured, causes found and not found); docs/dictation.md, docs/engines.md, docs/config.md and CHANGELOG updated.

QA follow-up (QA SHIP at 9b05c7e, P2 F1, memory bug/runtime-errors/dictation-user-with-stt-idle-seconds-2026-11-02):
- F1 fixed: a newly spawned engine process clears `last_recognized` (crates/dettivo-speech/src/supervisor.rs), so the 60 s warm-up skip counts only takes the running process served. A reload after an idle unload or a crash now warms even within 60 s of a take; a model change inside one live process keeps ADR 0084's skip. Red-then-green test `a_reload_after_an_idle_unload_warms_despite_a_recent_take` (crates/dettivo-speech/tests/support/warm_up.rs), committed failing in d8666cf before the fix d663fa9. ADR 0084 and 0098 say so.
- Stale bench constant: the warm-up lengths moved to `dettivo_engine_proto::warm_up` (`SAMPLES`, `PARAKEET_SAMPLES`, `samples(binary)`), used by both the supervisor's preload and the bench's `EngineClient::warm_up`, which now sends Parakeet 90 s (6840654). docs/qa.md and ADR 0098 updated (da4155f).

Decisions:
- The level stage stays at the release: its DC offset is the whole take's mean, and it costs 3.4 ms at 120 s (measured).
- Review fix: a cancelled take's check thread is joined before the session ends, because a detached piece could reset the next take's VAD stream (`a_cancelled_take_check_ends_before_the_next_take_starts`).
- Whisper's 30 s warm-up is unchanged. Its first speech take on an empty cache stays ADR 0084's accepted exception (fn-89's data point).

Follow-ups (not built):
- `dettivo-session` test `machine_exits::cancel_works_in_every_state` failed once under parallel load and passed 5 of 5 alone. It waits a fixed 150 ms for `Transcribing`, which predates this change.

stage: impl-review - ran (codex gpt-6.1-sol high: 3-lens panel NEEDS_WORK (cancel race, 30 s median claim), fixed in ff327f8, re-review SHIP)
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: plan-sync - skipped(config: planSync.enabled != true)
stage: impl-review (QA follow-up) - ran (codex gpt-6.1-sol high: 3-lens panel over d1af8f3..6840654, SHIP; one P3 doc finding fixed in da4155f)
## Evidence
- Commits: 159cdc2429f14674df7e1aae4e9191ff5291b001, 7429e4c4fb568cb19d52f0fc7c846d259317dae4, ff327f8800d5baefd4471c37c5550e9a3cc3756e, bdb7f7e4126456abe965a2cd6dbdf2d927e507f4, 9b05c7edfff234efae7d27313c444f05c6445943, d8666cf68a31f996573d604d3b68563726679b66, d663fa97a1c03bbd06fca5c637b01820435a9c76, 68406541978e5cb27cb85be4f00fa4e45743262d, da4155f1cc64b6332ae1f917fd420f0be439ff07
- Tests: just build test lint (xvfb-session, suite_rc=0), cargo test -p dettivo-speech -p dettivo-session -p dettivod (suite_rc=0), cargo test -p dettivo-engine-whisper --test vad --test vad_stream, daemon bench ~/.cache/agent-work/fn90 (docs/reports/benchmarks/release-to-text-2026-11-02.md), cargo test -p dettivo-engine-proto -p dettivo-speech -p dettivod -p dettivo-qa (isolated env, suite_rc=0, QA follow-up), just lint (lint_rc=0), rustup run 1.99.0 cargo clippy --workspace --all-targets -- -D warnings (rc=0)
- PRs: https://github.com/gmickel/dettivo-linux/pull/50