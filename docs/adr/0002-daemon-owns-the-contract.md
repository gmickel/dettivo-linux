# 0002. The daemon owns the contract and every interface is a client

Status: Accepted 2026-09-03

## What this gives you

Hold F9 in any app and dictation works whether or not a Dettivo window is open. The CLI, the bar panel, the app, MCP and the REST shim all talk to the same running `dettivod`, so state is never split between processes and a compositor binding is a one-line `exec`.

## Situation

On macOS the app process hosts the IPC server. On Linux the compositor drives push-to-talk by running a short command on key press and release, and Omarchy users expect no window to be involved. systemd user sessions offer socket activation, which starts a service on the first connection to its socket.

## Decision

`dettivod` is a systemd user service that owns audio capture, session state, persistence, the model catalogue and the JSON-RPC server on `$XDG_RUNTIME_DIR/dettivo/dettivo.sock`. A `dettivod.socket` unit activates it on first use. The app, OSD, CLI, MCP server and Omarchy plugin are clients. The REST shim runs inside the daemon, loopback only, off by default.

## Consequences

- Any client, including `dettivo dictation start` bound to a key, starts the daemon if it is not running; no client carries launch logic.
- Peer authentication uses `SO_PEERCRED`, so only processes of the same uid can connect; an optional token mode matches the macOS hardened mode.
- The daemon never renders UI. On Omarchy no resident Dettivo UI process runs at all; the bar plugin hosts the OSD (ADR 0010).
- A daemon crash ends every session; the meeting journal and the engine process isolation (ADR 0003) bound the damage to what was in flight.
