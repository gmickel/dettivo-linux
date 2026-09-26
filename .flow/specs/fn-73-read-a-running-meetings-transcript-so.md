# Read a running meeting's transcript so far

## Goal & Context

Agents such as Claude Code or Codex can coach you live in a meeting: every minute or two they read the newest transcript and suggest what to say, what is still open, and when you are being addressed. On Linux an agent that attaches late never sees what was said before it attached. Today every request-style read returns the stored row, and the row's segments stay empty until the meeting stops:
- `dettivo meetings segments <id>` calls `meetings.get` (`crates/dettivo-cli/src/meetings_segments.rs:103-108`).
- The live finals are copied onto the row only at Stopped (`crates/dettivo-meeting/src/worker_end.rs:430-437`).
- The live tail lives only in the capture worker (`crates/dettivo-meeting/src/live.rs:74`).

The only live source is the `meeting.segment` event stream (`--follow`), which starts at subscription and has no replay. MCP and REST see nothing live either (`docs/mcp.md:60`; REST serves no events).

On macOS, agents read the app's `live-checkpoint.json`. It holds the whole meeting so far, final and provisional segments, updated about a second after each change. Mac IPC has no live method either (`IPCService.swift:900-926`). The Linux checkpoint holds finals only and is written every 15 s.

This spec lets any agent read the transcript so far at any time, and then read only what is new.

## Approach

- **A Linux-addition method `meetings.segments`** with `{ meeting_id, since? }`. For the active meeting it reads the live tail from the capture worker: finals in order, the current provisional tail per source flagged provisional, and a cursor. For any other meeting it returns the stored row's segments. `meetings.get` keeps its contract meaning.
- **The cursor.** Finals are append-only, so `since` returns only the finals after the cursor, plus the current provisional tail, which is small and replaced as a whole. Segment ids match the `meeting.segment` event payload (`you-12`, `you-p1`), so a client can combine a snapshot with the event stream.
- **The CLI.** `dettivo meetings segments <id>` prints the transcript so far during a meeting, with provisional lines marked. `--since <cursor>` prints only what is new, and `--json` includes the cursor. `--follow` subscribes first, then prints the backlog, then streams, with no gap and no duplicates.
- **MCP and REST.**
  - MCP gets a tool that returns the same snapshot, with `since`.
  - REST gets it through the generic method mapping.
  - Both stay behind the existing auth, like every other read.
- **Docs and a recipe.**
  - `docs/meetings.md`, `docs/mcp.md` and `docs/api/linux-deltas.md` describe the method.
  - `docs/guides/agents.md` gains a short "live meeting copilot" recipe: find the active meeting, read the backlog, poll with `--since` every minute or two or follow the stream, and never stop, cancel or delete the meeting.

## Quick commands

- `just build test lint`

## Acceptance

- **R1:** While a meeting records, `meetings.segments` returns every final live segment so far, in order, plus the current provisional tail flagged as provisional. It returns within 100 ms on a two-hour meeting's tail, without touching capture or the checkpoint.
- **R2:** `since` returns only finals after the cursor plus the current provisional tail. Polling with the returned cursor never skips or repeats a final segment across the meeting, including across the Stopping and Stopped transitions into the stored row, and a test covers it.
- **R3:** For a meeting that is not recording, it returns the stored row's segments, the same as `meetings.get`. Unknown ids get the usual not-found error.
- **R4:** `dettivo meetings segments <id>` prints the backlog during a recording, with provisional lines marked. `--since` and `--json` expose the cursor. `--follow` started mid-meeting prints the backlog, then streams without a gap or a duplicate, and a daemon test covers the race.
- **R5:** An MCP tool and the REST route return the same snapshot and cursor, and tests cover both.
- **R6:** A contract fixture for the method, its catalog entry marked as a Linux addition, and the docs listed above are updated. `docs/guides/agents.md` carries the live-copilot recipe.
- **R7:** Reading never mutates the meeting: nothing is written and capture is not slowed. A test drives reads at a high rate during a recording fixture and checks the takes and the checkpoint are unchanged.

## Boundaries

- No change to `meetings.get`'s contract meaning, the checkpoint format or its interval.
- No macOS change in this repo. Mac agents keep reading `live-checkpoint.json`. A Mac IPC equivalent would be a sibling spec on `gmickel/dettivo` if wanted.
- No coaching logic in Dettivo. The agent skill owns that.

## Decision Context

Gordon asked for this on 2026-09-26 for a live-meeting skill (`~/work/GordonsVault/.claude/skills/live-meeting`). Today it tells users on Linux that history before attaching is "not available until the meeting stops". Fixing it in the product removes that caveat for every agent, which is better than a workaround in the skill.
