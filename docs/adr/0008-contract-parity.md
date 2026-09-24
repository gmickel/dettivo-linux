# 0008. The macOS IPC v1 contract is normative, deltas live in one file, fixtures prove conformance

Status: Accepted 2026-09-03

## What this gives you

A skill, script or MCP client written against Dettivo on macOS works against Dettivo on Linux with the same method names, tool names, resource URIs, error codes and exit codes, and the one place to look for differences is `docs/api/linux-deltas.md`.

## Situation

The macOS repository owns `docs/api/dettivo-ipc-v1.md` and `dettivo-rest-v1.md`. Its socket speaks newline-delimited JSON-RPC 2.0 with numeric error codes from -32010 to -32015, its MCP server exposes 19 tools and 5 resource templates with hard output bounds, and its CLI uses exit codes 0 to 5. The Windows port grew to 90 methods and 89 tools without a parity document and reconstructed its drift by hand; its useful additions are the `speech.providers.list` and `speech.selection.*` methods and the `acknowledge_meeting_disclosure` parameter.

## Decision

The two macOS contract documents are copied into `docs/api/` unchanged with the commit they came from. `dettivo-proto` holds the Rust types. `docs/api/linux-deltas.md` lists every addition, omission and behavioural difference with its reason and capability flag, and gives every contract item a disposition (implemented, covered elsewhere, deferred, blocked) against pinned macOS and Windows commits. Reserved namespaces exist in the router and return `NOT_IMPLEMENTED`. Provider and model management reuses the Windows `speech.*` names. A golden fixture suite replays request and response pairs per method and error over the socket in CI.

## Consequences

- `system.capabilities` declares what Linux implements; agents branch on flags rather than on platform.
- Linux additions (`events` topics `audio.level`, `model.download`, `engine.state`, `meeting.segment`; `json` meeting export; `platform` block; `speech.models.*`) are proposals for the other ports, recorded as deltas.
- The fixture suite is written so it can be pointed at a macOS or Windows socket later.
