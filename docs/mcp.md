# MCP

One line makes Dettivo a tool call in Claude Code, Codex, Cursor and Claude Desktop: `dettivo mcp config --host <host> --write` registers `dettivo mcp serve`, and from then on an agent lists your transcripts, searches them, inserts one into the focused window or starts a dictation through the same nineteen tools and the same resource URIs the macOS server exposes, plus `get_meeting_segments` for a meeting that is still running. Instructions and skills written against Dettivo on macOS work here unchanged (ADR 0019, ADR 0008).

## Connecting a host

```
dettivo mcp config --host claude-code            # prints the `claude mcp add` line
dettivo mcp config --host claude-desktop --write # merges into ~/.config/Claude/claude_desktop_config.json
dettivo mcp config --host cursor --write         # merges into ~/.cursor/mcp.json
dettivo mcp config --host codex --write          # merges into $CODEX_HOME/config.toml or ~/.codex/config.toml
dettivo mcp check                                # the server, the tool count and a served handshake per framing against the daemon
```

Without `--write` the entry is printed with the path it belongs in. `--write` merges it into the file and keeps every other server and every other key; run it again after a change and the entry is replaced in place. `--name` changes the server name (default `dettivo`), `--command` pins the command path (default `~/.local/bin/dettivo` when it exists, then the binary that ran), and `--socket` is carried into the entry as `DETTIVO_IPC_SOCKET`. For Claude Code the printed form is the `claude mcp add --scope user` line; `--write` produces a project `.mcp.json` in the current directory instead.

The entry every host receives:

```json
{"type": "stdio", "command": "/usr/bin/dettivo", "args": ["mcp", "serve"]}
```

`dettivo mcp check` proves the whole path a host will take: it asks the daemon for its capabilities and version, then runs `dettivo mcp serve` as a child once per framing (line-delimited, then Content-Length) and speaks MCP to it, `initialize`, `notifications/initialized`, `tools/list` and the `get_status` tool, within a bounded time. It prints the server name and version, the socket and auth mode, how many tools the daemon exposes (and which meeting tools are hidden), the resource template count and, per framing, the steps that answered. It exits 2 when the daemon is unreachable and 1 when a framing's handshake fails, naming the step, so a host configuration can be verified before the agent ever starts.

## Hardened mode

When the daemon runs with `[ipc] auth_mode = "peer_token"`, the server needs the token: `dettivo mcp config --hardened` (or `[mcp] hardened = true` in `config.toml`) writes a `DETTIVO_IPC_TOKEN = "<set-me>"` placeholder into the entry's environment for you to fill in, and the server also honours `--token` and `--token-file`. A tool called with a token the daemon refuses answers with the macOS text:

```
<the daemon's message> (UNAUTHORIZED_CLIENT)
Action: If IPC hardened mode is enabled, provide DETTIVO_IPC_TOKEN (or --token/--token-file) and retry.
```

## The server

`dettivo mcp serve` (and the `dettivo-mcp` binary, the same code) speaks MCP over standard input and output, identifies as `dettivo-mcp` 1.0.0, echoes the client's protocol version, and implements `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/read` and `resources/templates/list`. Notifications are accepted and answered with nothing.

The framing is whatever the client speaks first. A first byte of `{` or `[` means one JSON message per line; a `Content-Length:` header means LSP-style framing. The server answers in that framing for the session. A later framing switch is refused and its complete payload is drained without dispatch. Valid subsequent messages in the original framing remain usable; malformed or oversized switch headers that cannot be drained safely end the session (ADR 0055). Standard output carries protocol only; `DETTIVO_MCP_DEBUG=1` writes every message to standard error.

Each tool is one translation to the daemon: the server holds no state of its own, opens one socket connection per daemon call, and resolves the socket and the token exactly as the `dettivo` command does (`--socket`, `DETTIVO_IPC_SOCKET`, the default under `$XDG_RUNTIME_DIR`; `--token`, `--token-file`, `DETTIVO_IPC_TOKEN`). A daemon that is not running does not stop the server: every tool answers the actionable text and the session continues.

```
Dettivo daemon/IPC unavailable (socket: /run/user/1000/dettivo/dettivo.sock, ...)
Action: Start the Dettivo daemon (systemctl --user start dettivod.socket) and retry. If needed, override socket with DETTIVO_IPC_SOCKET or --socket.
```

## The tools

The nineteen macOS tools, with the macOS names, input schemas (the contract's snake_case keys) and result shapes, and one Linux addition, `get_meeting_segments`. A tool result carries the daemon's answer twice: as `structuredContent` and as pretty JSON in a text block. A tool that fails answers an `isError` result whose text says what to do; only a malformed request is a JSON-RPC error.

| Tool | Daemon method | Notes |
|---|---|---|
| `get_status` | `system.health` + `system.capabilities` | The health fields plus a `capabilities` block, so one call tells an agent which flags are on. |
| `list_transcripts` | `transcripts.list` | `kinds` (default both), `limit` (1 to 50, default 20), `cursor`. |
| `get_transcript` | `transcripts.get` | With `kind`, that kind; without, dictation first and meeting second. |
| `search_transcripts` | `transcripts.search` | `query` (control characters refused), `kinds`, `limit` (default 10). |
| `get_latest_transcript` | `transcripts.latest` | `kind` dictation, meeting or any (default). |
| `start_dictation` | `dictation.start` | `language`, `mode` (default raw). |
| `stop_session`, `cancel_session` | `dictation.stop` / `dictation.cancel`, or `meetings.stop` / `meetings.cancel` when `meeting_id` is given | |
| `start_meeting`, `list_meetings`, `get_meeting`, `search_meetings` | `meetings.*` | Hidden from `tools/list` while `formats.meeting_export` is empty. `start_meeting` takes `acknowledge_meeting_disclosure` and answers the running capture; `get_meeting` carries the capture-level fields and, once the meeting completed, its transcript ([docs/meetings.md](meetings.md)). |
| `get_meeting_segments` | `meetings.segments` | Linux addition (ADR 0071), hidden with the meeting tools. `meeting_id`, and `since` (the `cursor` of an earlier answer). Answers the transcript so far, live while the meeting records and finalises, with the finals after `since`, the provisional tail and the next cursor; `reset = true` says the stored transcript replaced the live one ([docs/meetings.md](meetings.md#the-transcript-so-far)). |
| `import_audio` | `transfer.begin`, `transfer.chunk`, `transfer.commit`, `transcripts.import` | `file_path` (must exist and be non-empty), `target_kind`, `language` (default en), `mode`. Reads and hashes bounded chunks; failure cancels the transfer. Answers at once with the running job; the item completes through the chunked pipeline ([docs/history.md](history.md)) and `get_transcript` then carries its segments. |
| `export_transcript` | `transfer.begin`, `transcripts.export`, `transfer.pull` | Writes a sibling temporary file and publishes `out_path` only after checked completion, then answers `{transfer_id, out_path}`. Otherwise answers `{transfer_id, preview}`, retaining at most 80,000 bytes and 20,000 text characters; partial previews explicitly request `out_path`. Any failure, including the final acknowledgement, cancels the transfer and preserves an existing destination. |
| `insert_transcript` | `insert.perform` | `text` or `source_ref`, `mode` (default polish), the target guards. |
| `list_polish_rules`, `set_polish_app` | `polish.rules.get`, `polish.apps.set` | The Polish rules and app profiles (docs/polish.md). |
| `create_automation_job`, `list_automation_jobs` | `automation.jobs.*` | Answer the automation guidance while `automation.jobs` is false. |

A tool the daemon has not implemented answers the daemon's `NOT_IMPLEMENTED` message and an `Action:` line naming the spec that brings it and the capability flag to check. An unknown tool name answers `Unknown tool: <name>` followed by the tools the server knows.

## The resources

`resources/templates/list` names six templates; `resources/list` names the current status, the speech providers and the newest fifty history items as `transcript://<id>` (or `meeting://<id>`).

Malformed percent escapes and invalid decoded UTF-8 in a resource URI return `INVALID_PARAMS`. Valid Unicode remains intact, and a malformed request leaves the session available for subsequent requests (ADR 0050).

| URI | Answers |
|---|---|
| `status://current` | The `get_status` payload. |
| `transcript://{id}` | `transcripts.get`, dictation first and meeting second. |
| `meeting://{id}` | `meetings.get`, the seeded sample meeting among them. |
| `transcripts://search/{query}?kinds=dictation,meeting&limit=10` | `transcripts.search`; the query is percent-decoded. |
| `transcripts://latest/{kind}` | `transcripts.latest`; an empty kind means any. |
| `speech://providers` | `speech.providers.list` (a Linux addition from the Windows port). |

Every body is `application/json` text with the bounds applied.

## Bounds

Every tool result and resource body is cut to the macOS bounds before it leaves the server. The limits are 50 items per list, 100 keys per object, 20 000 characters per text, nesting depth 8, and 2 000 000 bytes per message in either direction (`[mcp] max_message_bytes`). A text ends in `[truncated N chars]`, an object gains `_truncated_keys`, a value past the depth becomes `[truncated: max depth reached]`, and a shortened tool payload carries `_truncated: true`. The complete serialized response is then checked, including its ID and duplicate text/structured content. An oversized result becomes a bounded `RATE_LIMITED_LOCAL` error (`-32016`) retaining the request ID. If even a compact error and that ID cannot fit, the session ends without writing an oversized message. An oversized incoming message is drained and answered when the error fits (ADR 0055).

## Proving it

`cargo test -p dettivo-mcp` drives the server binary in both framings against a seeded daemon through the crate's harness (every tool and resource above, the unknown tool, the guidance answers, and `import_audio` live through tiny.en when the test model is present, skipped with the reason otherwise), checks the `initialize` bytes against `crates/dettivo-mcp/fixtures/framing/`, the tool definitions against `crates/dettivo-mcp/fixtures/tools/`, the bounds against a daemon stand-in that answers oversized payloads, and the unavailable and hardened texts. `just qa-mcp` (`dettivo-qa mcp`) runs the same harness in the QA rig and reports one row per step and framing; the contract replay's report carries the same section, and the CI `drive` job runs both. `cargo test -p dettivo-cli --test mcp` holds the host goldens under `crates/dettivo-cli/tests/goldens/mcp-*.txt` and the `--write` merges.
