# 0019. The MCP server is a hand-rolled JSON-RPC layer over stdio that maps the macOS tools one to one onto daemon methods

Status: Accepted 2026-09-04

Amended by [0050](0050-private-transfers-authenticated-adapters-and-isolated-qa-processes.md), which rejects malformed percent escapes without terminating the MCP session.

## What this gives you

`dettivo mcp config --host claude-code|claude-desktop|cursor|codex --write` connects an agent to Dettivo in one line, and the agent sees the same nineteen tools, six resource URIs, bounds and error texts it sees on macOS, so a skill written for one port drives the other. The server is a few hundred lines that never go stale behind a framework, and the harness proves both wire framings against a live seeded daemon on every CI run.

## Situation

The macOS port ships an MCP server inside its CLI (`DettivoCLI/MCP/`): nineteen tools with fixed names and JSON schemas, five resource templates, a stdio transport that detects line-delimited or `Content-Length` framing from the first bytes and mirrors it, bounds of 50 items, 100 keys, 20 000 characters, depth 8 and 2 000 000 bytes with truncation markers, and actionable texts for a daemon that is down and a token that is refused. The masterplan (FR-S4, FR-S8, QA-7) asks Linux for the same surface plus `claude-code` as a host, `speech://providers` from the Windows port, and a harness in both framings. The Rust MCP SDKs on offer bring an async runtime, a schema derive layer and a protocol-version policy of their own; the daemon's router (ADR 0002) already shows that a JSON-RPC layer over `serde_json` is small and easy to keep in step with a contract. The CLI, MCP server and QA runner may depend on `dettivo-proto` only (`tools/xtask/src/edges.rs`), and the QA rig needs to drive the server as a client would.

## Decision

`crates/dettivo-mcp` implements the protocol by hand over `serde_json`: a framing-detecting reader, the seven MCP methods (`initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `resources/templates/list`), the macOS tool catalog ported verbatim, the six resource templates, the bounds, and a one-connection-per-request daemon client with the CLI's socket and token resolution. No MCP framework crate. Every tool is one translation to the daemon methods in the table below; the server holds no state and duplicates no business logic.

| Tool | Daemon |
|---|---|
| `get_status` | `system.health` merged with `system.capabilities` under `capabilities` |
| `list_transcripts`, `get_transcript`, `search_transcripts`, `get_latest_transcript` | `transcripts.list`, `transcripts.get` (dictation first, meeting second), `transcripts.search`, `transcripts.latest` |
| `start_dictation`, `stop_session`, `cancel_session` | `dictation.start`, `dictation.stop` / `meetings.stop` and `dictation.cancel` / `meetings.cancel` by the presence of `meeting_id` |
| `start_meeting`, `list_meetings`, `get_meeting`, `search_meetings` | `meetings.*`; hidden while `formats.meeting_export` is empty |
| `import_audio` | `transfer.begin`, `transfer.chunk`, `transfer.commit`, `transcripts.import` |
| `export_transcript` | `transfer.begin`, `transcripts.export`, `transfer.pull`, `transfer.commit`; `transfer.cancel` on failure |
| `insert_transcript` | `insert.perform` (`expected_target_pid` sent as the contract's string) |
| `list_polish_rules`, `set_polish_app` | `polish.rules.list`, `polish.apps.set` |
| `create_automation_job`, `list_automation_jobs` | `automation.jobs.create`, `automation.jobs.list` |

Resources map the same way: `status://current` to `get_status`, `transcript://{id}` to `transcripts.get`, `meeting://{id}` to `meetings.get`, `transcripts://search/{query}` to `transcripts.search`, `transcripts://latest/{kind}` to `transcripts.latest`, `speech://providers` to `speech.providers.list`.

The crate is a library with a thin binary. `dettivo mcp serve` runs the same library in-process, so `dettivo-cli` gains the edge to `dettivo-mcp`; `dettivo-qa` gains the same edge for the crate's `harness` module, which drives a spawned server through its pipes in one framing and runs the representative steps. Both crates still reach the daemon through `dettivo-proto` only.

`dettivo mcp config` renders one server entry (`{type: stdio, command, args: [mcp, serve]}`, `env` with `DETTIVO_IPC_SOCKET` and, hardened, `DETTIVO_IPC_TOKEN = "<set-me>"`) into each host's file with the other servers kept: JSON `mcpServers` for Claude Desktop and Cursor, a `claude mcp add` line or a project `.mcp.json` for Claude Code, and a `[mcp_servers.<name>]` table through `toml_edit` for Codex. `[mcp] hardened` and `[mcp] max_message_bytes` live in `config.toml`; the server reads the cap through `config.get` and falls back to the contract's 2 000 000 when no daemon answers.

## Consequences

- The macOS texts are kept where the remedy is the same (`Action: If IPC hardened mode is enabled, provide DETTIVO_IPC_TOKEN (or --token/--token-file) and retry.`, the timeout line, `Unknown tool: <name>`); the unavailable line names `systemctl --user start dettivod.socket` instead of the app, and a `NOT_IMPLEMENTED` answer gains an `Action:` line naming the spec that brings the tool (`crates/dettivo-mcp/src/messages.rs`). The unknown-tool text appends the tools the server knows.
- Two additions on top of macOS, both registered in `docs/api/linux-deltas.md`: a shortened list sets the top-level `_truncated: true` (macOS cuts lists silently, so an agent could not tell), and a message over the byte cap is answered with `RATE_LIMITED_LOCAL` (`-32016`) naming the cap and the session continues, where macOS ends the session. `get_status` carries a `capabilities` block beside the health fields.
- The meeting tools are gated exactly as on macOS, on `formats.meeting_export`. The Linux daemon declares meeting formats, so all nineteen tools are listed and the four meeting tools answer the daemon's `NOT_IMPLEMENTED` with the meetings guidance until the meetings spec lands; a daemon without meeting formats lists fifteen, which the unit test covers.
- The server is synchronous: one message at a time, one daemon connection per call, no streaming results. A slow daemon call blocks the next message for at most `--timeout-ms` (default 5000).
- Hand-rolled means every new MCP capability (prompts, subscriptions, sampling) is an explicit change here, with the macOS server as the reference; the protocol version is echoed, never negotiated.
- Proof: `crates/dettivo-mcp/tests/harness.rs` runs every step in both framings against a seeded daemon, the `initialize` bytes match `fixtures/framing/`, the tool definitions match `fixtures/tools/`, `tests/bounds.rs` proves the cuts and the byte cap against a daemon stand-in, and `just qa-mcp` runs the harness in CI's `drive` job.
