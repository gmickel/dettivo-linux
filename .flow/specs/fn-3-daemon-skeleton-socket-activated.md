# Daemon skeleton: socket-activated dettivod with peer auth, config and the CLI

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 1): "Rust direction recorded"
> user (turn 13): "surely we only need 1 step potentially for keyboard shortcuts and the rest should be config file heavy etc. we don't have the same bullshit permission issues like on mac"
> user (turn 13): "ALL needs to be able to be configured via config files as is the linux way"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> user (turn 20): "yes"

## Goal & Context

<!-- Goal & Context: 40% [user], 30% [paraphrase], 30% [strategy] -->

Any client, including a one-line compositor binding, reaches a running Dettivo: the first connection to the socket starts the daemon, only the same user's processes may talk to it, and everything it does is governed by a configuration file the user can read, edit and version. This spec delivers that resident core with the `system.*` methods live, every other method answering `NOT_IMPLEMENTED`, the configuration layer, and the `dettivo` command that talks to it. [paraphrase]

Linux has no permission dialogs to walk through, so configuration lives in files "as is the linux way", and the daemon has to treat the file as the source of truth from its first version. [user]

## Architecture & Data Models

<!-- Architecture & Data Models: 70% [paraphrase], 30% [inferred] -->

- `dettivod` is a tokio process owning a JSON-RPC 2.0 server over a Unix socket in the user's runtime directory, with the directory at mode 0700 and the socket at 0600. A systemd socket unit activates the service unit on first connection; the service also runs standalone for development. [paraphrase]
- Peer authentication reads `SO_PEERCRED` on every connection and refuses a different uid with `UNAUTHORIZED_CLIENT`. Token mode, when enabled, additionally requires the shared token resolved in the macOS order: environment variable, Secret Service item, then a mode-0600 token file. `system.capabilities` reports `peer` or `peer_token`. [paraphrase]
- The router dispatches by method name against the contract types, implements `system.ping`, `system.health`, `system.version` and `system.capabilities`, and answers every other contract method with `NOT_IMPLEMENTED` in the reserved shape until its spec lands. [paraphrase]
- Configuration is one TOML file in the user's config directory: loaded at start, watched for changes, overridable by environment variables for socket path, config path, data directory and QA mode. Writes go through the daemon and preserve comments and ordering. Non-configuration state (window geometry, acknowledgements, first-run progress) lives in a separate state file in the state directory. [user]
- Exactly one daemon per user session; a second start exits with status 1 and a message naming the running instance. [paraphrase]
- Logs go to standard error under systemd (journald) with structured levels, and never contain transcript text, audio content or prompts. [paraphrase]
- The `dettivo` binary maps commands to contract methods with human output by default and `--json` for machines, honours socket and token overrides, uses the contract's exit codes, and adds `config` (get, set, unset, path, edit, validate, print-default) and a first `doctor` that reports service, socket, config and platform facts. [paraphrase]

## API Contracts

<!-- API Contracts: 80% [paraphrase], 20% [inferred] -->

- `system.health` returns `ok`, `recording_state` (`idle`, `dictation`, `meeting`), `active_jobs` and `uptime_seconds`; `system.version` returns `api_version`, `app_version`, `build`; `system.capabilities` returns the flag set from the contract-types crate with the Linux additions. [paraphrase]
- Exit codes: 0 success, 1 runtime failure, 2 daemon unavailable, 3 permission denied, 4 invalid arguments, 5 timeout. Overrides: `DETTIVO_IPC_SOCKET`, `DETTIVO_IPC_TOKEN`, `--socket`, `--token`, `--token-file`, `--timeout-ms`, `--quiet`. [paraphrase]
- `dettivo config get <key> --json` returns the effective value and its source (default, file, environment). `dettivo config print-default` emits the fully commented default file. [paraphrase]
- Environment overrides: socket path, config path, data directory, QA mode. [paraphrase]

## Edge Cases & Constraints

- A stale socket file from a crashed daemon is unlinked on bind; a live daemon on the path makes the second start exit 1. [inferred]
- A malformed configuration file fails validation with the key and line named; the daemon starts with defaults and reports the failure through `system.health` rather than refusing to start. [inferred]
- Socket readiness after activation is measured and must stay under 300 ms on the development machine. [paraphrase]
- Graceful shutdown closes the socket and exits within a bounded time; there is nothing in flight yet to journal. [paraphrase]

## Acceptance Criteria

- **R1:** With the systemd user units installed, `dettivo status ping` on a machine with no running daemon starts it through socket activation and returns success, and the socket is ready within 300 ms of activation. Errors: activation failure returns exit code 2 with the unit name in the message. [paraphrase]
- **R2:** A connection from a different uid is refused with `UNAUTHORIZED_CLIENT`; in token mode a connection without the token is refused the same way, and `system.capabilities` reports the active mode. Errors: no error surface beyond the refusal. [paraphrase]
- **R3:** The `system.*` fixtures from the contract suite pass against the live socket, and every other contract method returns `NOT_IMPLEMENTED` in the reserved shape. Errors: a malformed request line returns `INVALID_PARAMS`; an oversized line is rejected without disconnecting. [paraphrase]
- **R4:** A second daemon start exits with status 1 and names the running instance; a stale socket file does not prevent a clean start. Errors: no error surface beyond these two. [paraphrase]
- **R5:** The configuration file is loaded at start, changes are picked up without restart, environment overrides win over the file, `config set` preserves comments and ordering, and non-configuration state is written only to the state file. Errors: an invalid key or value fails `config set` and `config validate` with the key named; the daemon never writes a file it could not parse. [user]
- **R6:** The CLI maps every implemented method 1:1, supports `--json` and `--quiet`, honours the socket and token overrides, and returns the contract's exit codes, including 2 when the daemon is unreachable and 5 on timeout. Errors: unknown commands return 4 with usage. [paraphrase]
- **R7:** Logs at every level contain no transcript text, audio paths, audio content or prompts, verified by a test that exercises every log site with marker strings. Errors: no error surface. [paraphrase]

## Boundaries

- No audio, engines, insertion, history or events; only the resident core and its control surface. [paraphrase]
- No REST shim and no MCP server; they arrive with their own specs on top of this socket. [paraphrase]
- No GUI settings editor; the configuration file and the CLI are the surfaces here. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- Linux users expect to provision a machine from files, and the product has no permission walls to justify a wizard, so the configuration file is the source of truth from the first daemon version rather than a later addition. [user]
- The compositor drives push-to-talk by running a command on key press and release, which is only reliable when a resident process answers instantly; socket activation makes that true without any client carrying launch logic. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the socket, the error taxonomy and the exit codes are the macOS contract implemented on Linux. [strategy:Contract parity and agent surfaces]
- **Omarchy-native, beautiful by default:** a resident daemon behind compositor keys is how Omarchy's own tools work. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
