# MCP server: stdio transport, the nineteen tools, resources and client configuration

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-29 row: "`mcp-server-stdio-tools-resources-config` | phase 5 | depends on S-11 | FR-S4, S8; QA-7 | MCP harness both framings; bounds tests; `mcp config` goldens per host"
> masterplan FR-S4: "The MCP server runs over stdio through `dettivo mcp serve`, identifies as `dettivo-mcp` version 1.0.0, echoes the client's protocol version, autodetects line-delimited and `Content-Length` framing and mirrors it, exposes the 19 macOS tools (`get_status`, `list_transcripts`, `get_transcript`, `search_transcripts`, `get_latest_transcript`, `start_dictation`, `stop_session`, `cancel_session`, `start_meeting`, `list_meetings`, `get_meeting`, `search_meetings`, `import_audio`, `export_transcript`, `insert_transcript`, `list_polish_rules`, `set_polish_app`, `create_automation_job`, `list_automation_jobs`) and resources (`status://current`, `transcript://{id}`, `meeting://{id}`, `transcripts://search/{query}`, `transcripts://latest/{kind}`), adds `speech://providers` from Windows, and applies the macOS bounds (50 items, 100 object keys, 20 000 characters of text, depth 8, 2 000 000 bytes per message) with a `_truncated` marker. Meeting tools are hidden when `formats.meeting_export` is empty; automation job tools return `NOT_IMPLEMENTED` guidance while the capability is off. Unavailable daemon and hardened auth mismatch return the macOS actionable text."
> masterplan FR-S8: "`dettivo mcp config --host claude-desktop|claude-code|cursor|codex [--write] [--hardened]` prints or writes the client configuration, keeping the macOS command name and adding `claude-code`."
> masterplan QA-7: "The contract fixture suite covers every implemented method, every error code, reserved methods, both MCP framings and the engine protocol."
> masterplan 8.2: "`crates/dettivo-mcp` | Rust | MCP stdio server used by `dettivo mcp serve`."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Claude Code, Codex, Cursor and Claude Desktop reach Dettivo through one command. `dettivo mcp serve` is the macOS MCP server on Linux: the same nineteen tools and six resources over stdio with both framings, the same bounds and truncation marker, the same actionable messages when the daemon is down or the token does not match, meeting tools hidden until meetings exist and automation tools answering with guidance while the capability is off. Each tool is a thin translation to the daemon's IPC methods, so the contract fixtures already cover the behaviour and the MCP harness covers the framing, the schemas and the bounds. `dettivo mcp config` prints or writes each host's configuration, hardened when asked. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/dettivo-mcp` (depends on `dettivo-proto`, `dettivo-core`; a small hand-rolled JSON-RPC 2.0 MCP layer, no framework, following T13's reasoning): `transport` (stdin reader that sniffs the first bytes for `Content-Length:` and otherwise reads newline-delimited JSON, mirroring the detected framing on stdout; one message at a time, the 2 000 000 byte cap enforced before parsing), `protocol` (`initialize` echoing the client's protocol version, `dettivo-mcp` 1.0.0 as server info, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `ping`, notifications ignored), `tools` (the nineteen definitions with JSON schemas ported from the macOS server, each mapped to one IPC call and result shaping), `resources` (the six URI templates plus `speech://providers`), `bounds` (50 items, 100 object keys, 20 000 characters of text, depth 8 with `_truncated: true` markers, applied to every tool result and resource body), `client` (a blocking Unix socket client to the daemon with the token resolution order of the CLI, one connection per request, timeouts). [paraphrase]
- Tool mapping: `get_status → system.health + system.capabilities`, `list_transcripts → transcripts.list`, `get_transcript → transcripts.get`, `search_transcripts → transcripts.search`, `get_latest_transcript → transcripts.latest`, `start_dictation → dictation.start`, `stop_session → dictation.stop` (or `meeting.stop` when a meeting runs), `cancel_session → dictation.cancel`, `start_meeting|list_meetings|get_meeting|search_meetings → meeting.*` (hidden while `formats.meeting_export` is empty), `import_audio → transfer.* + transcripts.import` (base64 body bounded), `export_transcript → transcripts.export + transfer.pull`, `insert_transcript → insert.perform { source_ref }`, `list_polish_rules → polish.rules.get` (NOT_IMPLEMENTED guidance until the polish spec lands, then live), `set_polish_app → polish.apps.set`, `create_automation_job|list_automation_jobs → automation.*` guidance while off. [paraphrase]
- Messages: daemon unavailable answers the macOS text with the systemd hint; a token mismatch under hardened auth answers the macOS text naming the token sources. [paraphrase]
- `dettivo mcp serve` in the CLI crate spawns the server in-process (the CLI links `dettivo-mcp`); `dettivo mcp config --host claude-desktop|claude-code|cursor|codex [--write] [--hardened]` renders the host's file (the macOS templates, Linux paths: `~/.config/Claude/claude_desktop_config.json`, Claude Code's `claude mcp add` command line or `.mcp.json`, `~/.cursor/mcp.json`, `~/.codex/config.toml`), `--write` merges into the existing file preserving other servers, `--hardened` adds the token environment. Goldens per host. [paraphrase]
- Harness: `crates/dettivo-mcp/tests/harness.rs` drives the server binary through both framings with `initialize`, `tools/list`, `resources/list`, representative `tools/call` and `resources/read` against a live seeded daemon, and a bounds test with an oversized transcript; the QA replay gains `dettivo-qa mcp` running the same harness as an L1 step (`just qa-mcp`, in the CI contract job). [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- MCP protocol over stdio: `initialize` (server `dettivo-mcp` 1.0.0, capabilities tools and resources), `tools/list` (nineteen tools, meeting tools filtered by capability), `tools/call`, `resources/list`, `resources/read`, `ping`; framing mirrored. Fixtures under `crates/dettivo-mcp/fixtures/` per tool and per framing, checked by the harness. [paraphrase]
- Tool input schemas and result shapes are the macOS ones (ported verbatim into `tools/*.json`); a `_truncated` marker on bounded results. [paraphrase]
- CLI: `dettivo mcp serve`, `dettivo mcp config --host <host> [--write] [--hardened] [--json]`, `dettivo mcp check` (Linux addition: initialises against the daemon and prints the tool count and the framing). [inferred]
- Config: `[mcp] hardened = false`, `max_message_bytes` (default the contract's 2 000 000), documented. [inferred]

## Edge Cases & Constraints

- Framing switches mid-stream: refused; the server answers the framing it detected first. [inferred]
- A message over the byte cap: an error response with `RATE_LIMITED_LOCAL` semantics and the cap, the connection continues. [paraphrase]
- Daemon down mid-session: each tool call answers the actionable text; the server never exits on a daemon error. [paraphrase]
- `import_audio` bodies over the transfer limit: refused naming the limit before any chunk is sent. [inferred]
- `insert_transcript` for a meeting ref while meetings are off: `NOT_IMPLEMENTED` guidance. [inferred]
- Stdout is reserved for protocol; logs go to stderr at the configured level. [inferred]

## Acceptance Criteria

- **R1:** The harness passes both framings end to end against a live seeded daemon: `initialize` echoes the client's version and names `dettivo-mcp` 1.0.0, `tools/list` returns the nineteen tools minus the hidden meeting tools, `resources/list` the six plus `speech://providers`, and representative calls (`get_status`, `list_transcripts`, `search_transcripts`, `get_latest_transcript`, `insert_transcript` through the mock backend, `export_transcript`) return the macOS shapes. Errors: an unknown tool answers the protocol's error with the tool list. [paraphrase]
- **R2:** Bounds: a transcript over 20 000 characters, a list over 50 items, an object over 100 keys and nesting over depth 8 are cut with `_truncated` markers, and a message over the byte cap is refused naming the cap without ending the session. Errors: as stated. [paraphrase]
- **R3:** Daemon unavailable and hardened token mismatch answer the macOS actionable texts (golden strings); automation tools answer the guidance while the capability is off; `list_polish_rules` answers guidance until `polish.*` exists. Errors: as stated. [paraphrase]
- **R4:** `dettivo mcp config` output matches goldens for the four hosts, plain and hardened; `--write` merges into an existing file keeping other servers (unit test with a seeded file); `dettivo mcp check` prints the tool count and framing against the live daemon. Errors: an unknown host exits 4 naming the hosts. [paraphrase]
- **R5:** `just qa-mcp` runs the harness in the CI contract job through the QA rig (`dettivo-qa mcp`), reported per tool and framing, and the contract replay's report gains the MCP section. Errors: as stated. [paraphrase]
- **R6:** `docs/mcp.md` explains the server, the tools, the resources, the bounds, the hosts and the hardened mode; `[mcp]` keys are documented and printed by `config print-default`; an ADR records the hand-rolled protocol layer and the tool mapping. Errors: as stated. [paraphrase]

## Boundaries

- No REST shim (S-30). [paraphrase]
- No meeting or automation implementations; their tools are hidden or answer guidance. [paraphrase]
- No streaming tool results. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- The secondary persona lives in Claude Code and Cursor; one `dettivo mcp config` line makes transcripts and dictation a tool call, and parity with the macOS server keeps the agent instructions portable. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the macOS tool set, shapes, bounds and messages, verified by the harness in CI. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
