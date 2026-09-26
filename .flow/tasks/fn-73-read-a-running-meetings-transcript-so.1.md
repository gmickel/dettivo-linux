---
satisfies: [R1, R2, R3, R4, R5, R6, R7, R8, R9, R10]
---
# fn-73-read-a-running-meetings-transcript-so.1 Implement Read a running meeting's transcript so far

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
An agent that attaches to a running meeting late now reads everything said so far through `meetings.segments { meeting_id, since? }`, then polls with the returned cursor (`live:<n>` / `stored:<n>`) for only the new finals plus the flagged provisional tail. The session keeps the live tail readable from the start through stopping, stopped and transcribing, and drops it only after the finalisation stores the settled row, at which point `transcript` turns `stored` and the answer carries `reset = true` (ADR 0071). The CLI (`--since`, `--json` with the cursor, `--follow` that subscribes before it reads the backlog), the MCP tool `get_meeting_segments` and the REST route `/v1/meetings/segments` all answer the same snapshot.

- R1: `crates/dettivo-meeting/src/transcript.rs` unit test reads a synthetic two-hour tail (4,800 finals, 8 words each) whole in under 100 ms.
- R2, R10: `crates/dettivo-meeting/tests/live_read.rs::the_cursor_reads_every_final_once_from_the_recording_to_the_completed_row` polls through recording, stopping, transcribing and completed with no skipped or repeated final and exactly one reset. A red check (tail dropped at Stopped) failed it as intended. `crates/dettivod/tests/meetings_segments.rs` repeats the check against a live daemon.
- R3: contract fixtures `crates/dettivo-proto/fixtures/meetings/segments.json`, `segments.error-not-found.json` and `segments.error-invalid-params-cursor.json` replay against the seeded daemon.
- R4: the daemon test starts `dettivo --json meetings segments <id> --follow` mid-meeting and checks the backlog is non-empty, no final repeats, and the follower saw every final the poller did. `crates/dettivo-cli/tests/cli.rs` covers plain, `--json` and `--since` output.
- R5: REST fixture `crates/dettivo-rest/fixtures/routes/meetings.segments.json` (exact body) and the MCP harness step `get_meeting_segments` (cursor and `since`).
- R6: catalog entry marked `linux: true`, `system.capabilities.meetings.methods`, docs `docs/meetings.md`, `docs/mcp.md`, `docs/api/linux-deltas.md`, `docs/guides/agents.md` (live copilot recipe), ADR 0071.
- R7: `live_read.rs::reads_at_a_high_rate_leave_the_takes_and_the_checkpoint_unchanged` runs over 1,000 reads from two threads during a recording, checks the takes, the checkpoint and the journal are unchanged, and checks every sample fed during the reads is in the takes.
- R8, R9: every segment carries `source` and `source_type`, and the plain CLI line always shows the side. `provisional` is a separate array, and each of its entries is flagged. Streamed `--follow --json` events gain `source_type` in the CLI.

Changed on purpose: the plain `meetings segments` line now always shows the side with a leading mark column (` you    00:00.000-00:01.200  Hello.`), and `--json` prints the whole `meetings.segments` answer instead of the bare segment array. `tests/cli.rs` was updated for both. The MCP tool count is 20 in the count assertions.

Baseline: the first gate run hit an inherited flake, `dettivo-meeting` `transcription::a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed` (passed 3/3 when run alone). The rerun was fully green. Gate after the change: `just lint`, `just build` and `just test` ran as three foreground calls (the combined command exceeds one 600 s call) and all were green. No gate receipt was written because the exact combined command string never ran as one.

Follow-ups, not built: the `meeting.segment` event payload itself has no `source_type` (the CLI adds it when it streams). `docs/api/dettivo-ipc-v1.md` was left untouched because it is Mac-owned. The addition is registered in `linux-deltas.md`.

Tier: implementer claude-opus-5-5 (project routing block)

stage: impl-review - skipped(config: REVIEW_MODE=none)
## Evidence
- Commits: 8a26932ef8c00e57fa53d857db2e9f836ca48312
- Tests: baseline: green on rerun (first run hit the inherited flake dettivo-meeting transcription::a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed, 3/3 green isolated), MISE_JUST_VERSION=1.58.0 just lint, MISE_JUST_VERSION=1.58.0 just build, MISE_JUST_VERSION=1.58.0 just test, cargo test -p dettivo-meeting --test live_read --lib, cargo test -p dettivod --test meetings_segments, cargo test -p dettivod --test contract, cargo test -p dettivo-cli --test cli meetings_segments, cargo test -p dettivo-rest, cargo test -p dettivo-mcp
- PRs: