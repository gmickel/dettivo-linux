# The daemon and the command line

`dettivod` is the resident process every client talks to: the app, the bar, the compositor's key bindings, the `dettivo` command and, later, the MCP server (ADR 0002). It listens on a Unix socket in your runtime directory, starts on the first connection through systemd socket activation, and is governed by [config.toml](config.md).

## Starting it

With the user units installed the first client starts the daemon, and nothing else needs to know how:

```
systemctl --user enable --now dettivod.socket
dettivo status ping        # starts dettivod.service on demand
journalctl --user -u dettivod -f
```

The units are `systemd/user/dettivod.socket` and `dettivod.service`; packages install them under `/usr/lib/systemd/user/`. For a development checkout, `just install-units` installs them into `~/.config/systemd/user` with `ExecStart` pointing at `target/debug/dettivod`, and `just qa-activation` measures the activation path end to end (the budget is 300 ms from the first connection to the answer; on the development machine it measures well under that).

The daemon also runs standalone: `dettivod` binds the socket itself. Exactly one daemon runs per session. A second start exits with status 1 and names the running instance and its socket, and a socket file left behind by a crash is removed on the next start.

Command line: `dettivod [--socket PATH] [--config PATH]`, plus `--version` and `--help`. Everything else is configuration.

## The socket

- Path: `$XDG_RUNTIME_DIR/dettivo/dettivo.sock`, directory `0700`, socket `0600`. The service unit sets `DETTIVO_IPC_SOCKET` to the same path, so under systemd the daemon reports the socket the unit listens on regardless of `ipc.socket`.
- Protocol: JSON-RPC 2.0, one request per line, one response per line, as the IPC v1 contract specifies ([docs/api/dettivo-ipc-v1.md](api/dettivo-ipc-v1.md), Linux differences in [docs/api/linux-deltas.md](api/linux-deltas.md)).
- Authentication: every connection's peer uid is checked with `SO_PEERCRED`; another user's connection is refused with `UNAUTHORIZED_CLIENT`. In `peer_token` mode every request also carries `auth_token` (top level or inside `params`), compared against the token resolved from `DETTIVO_IPC_TOKEN`, the Secret Service, then the `0600` token file.
- Limits: a request line longer than `ipc.max_line_bytes` is answered with `INVALID_PARAMS` and the connection stays open. A line that is not JSON, not an object or not a JSON-RPC 2.0 envelope is `INVALID_PARAMS` too.
- Methods: `system.ping`, `system.health`, `system.version`, `system.capabilities`, the six `config.*` methods, `audio.devices`, `speech.providers.list`, `speech.selection.get`, `speech.selection.set`, the `speech.models.*` methods ([docs/models.md](models.md)), `speech.engines` ([docs/engines.md](engines.md)), the `dictation.*` methods and `events.subscribe`/`events.unsubscribe` ([docs/dictation.md](dictation.md)), `insert.perform` and the Linux additions `insert.undo` and `insert.target` ([docs/insertion.md](insertion.md)), the `transcripts.*` methods over both kinds with the Linux additions `transcripts.delete`, `transcripts.rerun`, `transcripts.stats` and `transcripts.cancel`, the five `transfer.*` methods ([docs/history.md](history.md)), the `meetings.*` methods with the Linux additions `meetings.recover`, `meetings.discard`, `meetings.disclosure.get` and `meetings.disclosure.acknowledge` ([docs/meetings.md](meetings.md)), and `hotkeys.status` ([docs/hotkeys.md](hotkeys.md)) answer today. Every other contract method answers `NOT_IMPLEMENTED` (`-32014`) in the reserved shape until its spec lands, and an unknown method name is `INVALID_PARAMS`.
- The history store opens before the socket: a database the daemon cannot read or migrate stops it with status 1 and the reason (the migration's name and its backup file) on standard error. A meeting a previous daemon left recording is promoted to `partial` before the socket listens ([docs/meetings.md](meetings.md)).
- `system.health.ok` is `false` while the configuration file does not parse; the daemon keeps serving on the last valid values (a file that is broken at start stops the start instead).

## Shutdown

SIGTERM or SIGINT stops accepting, lets open connections finish for up to `daemon.shutdown_timeout_ms`, removes the socket (when the daemon bound it) and the pid file, and exits 0. Under systemd the service unit's `TimeoutStopSec=10` caps that budget: a `shutdown_timeout_ms` above ten seconds is cut short by the unit, so raise both together.

## Logs

Logs go to standard error, which systemd forwards to the user journal. The level comes from `daemon.log_level` (`RUST_LOG` overrides it for one run). No level ever contains transcript text, audio paths, audio content, prompts, request parameters, client-chosen ids or configuration values: log lines carry method names from the static catalog, error codes, counts, and the file paths the daemon resolved itself. A test drives every request path with marker strings at `trace` and fails if one reaches the log.

## The `dettivo` command

Every command maps 1:1 to a contract method; `call` reaches any of them.

```
dettivo status ping | health | version | capabilities
dettivo config get [key] | set <key> <value> | unset <key> | path | edit | validate | print-default
dettivo insert --text <TEXT> [--mode raw|polish|clipboard_only] [--expected-target-bundle-id <APP_ID>] [--expected-target-pid <PID>]
dettivo polish test <TEXT> [--preset P] [--style S] [--bundle-id APP_ID] [--mode M] [--rule NAME]
dettivo polish rules list|add|set|remove ...
dettivo polish presets
dettivo polish apps list|set <APP_ID> <PRESET>
dettivo llm providers|endpoints|trust <URL>|test <TEXT>
dettivo insert --kind dictation|meeting --id <UUID>
dettivo insert undo | target
dettivo history list | get <id> | latest | search <words> | delete <id> | rerun <id> | export --out <file> | import <file>
dettivo meetings start | stop <id> | cancel <id> | status <id> | list | get <id> | segments <id> [--follow] | search <words> | delete <id> | recover <id> | discard <id> | disclosure [--acknowledge]
dettivo audio devices
dettivo setup hyprland|sway|niri [--stdout] [--check]
dettivo hotkeys status
dettivo mcp serve | config --host claude-desktop|claude-code|cursor|codex [--write] [--hardened] | check
dettivo rest serve [--port N] [--bind ADDR] | status | token
dettivo doctor
dettivo osd show <state> [--title ..] [--hint ..] [--engine ..] [--words ..] [--target ..] [--reason ..] [--action ..] [--level ..] | hide | status
dettivo app [open <route> | status]
dettivo call <method> [params-json]
```

`dettivo dictation start` takes `--expected-target-bundle-id <APP_ID>` and `--expected-target-pid <PID>` for a binding that captured the focused window itself; `dettivo setup` and `dettivo hotkeys status` are in [docs/hotkeys.md](hotkeys.md).
`dettivo rest serve` hosts the loopback REST shim as a process over the socket, `status` reports the listener and `token` names the token's source ([docs/rest.md](rest.md)).
`dettivo osd` and `dettivo app` do not go through the daemon: the first talks to `dettivo-osd` over `osd.sock` beside the daemon socket ([docs/osd.md](osd.md)), the second to `dettivo-app` over `app.sock` there ([docs/app.md](app.md)), so a script can show a pill state, raise the window or open a route in it while the daemon is down.

Global options: `--socket PATH`, `--token TOKEN`, `--token-file PATH`, `--timeout-ms N` (default 5000), `--json`, `--quiet`. The socket resolves from `--socket`, then `DETTIVO_IPC_SOCKET`, then the default; the token from `--token`, `--token-file`, then `DETTIVO_IPC_TOKEN`.

Output is human by default and the raw JSON result with `--json`; `--quiet` prints nothing on success, so a compositor binding can rely on the exit code alone. On a daemon error `--json` prints `{"error": {...}}` on standard output and one line on standard error.

Exit codes, from the contract:

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime failure, including a daemon error such as `NOT_IMPLEMENTED` |
| 2 | The daemon is unavailable; the message names the socket and `dettivod.socket` |
| 3 | Permission denied: `UNAUTHORIZED_CLIENT` or a socket the user cannot open |
| 4 | Invalid command or arguments, including `INVALID_PARAMS` from the daemon |
| 5 | Timeout |

`dettivo doctor` is the one report to paste into an issue: everything about this machine that decides whether dictation works and how fast, grouped for a person and in one stable shape for a script. It exits 0 when the daemon answers, the configuration is valid and no engine is degraded, 1 otherwise; an unreachable daemon still prints the machine rows. `--json` carries these keys on every machine (a value the machine lacks is `null`, a block keeps its keys), and `crates/dettivo-cli/tests/goldens/doctor.json` pins the shape the way the contract replay levels machine-specific results:

| Key | What it holds |
|---|---|
| `socket`, `service` | The socket path and whether it exists, `dettivod.socket` and `dettivod.service` as `systemctl --user` reports them (`unknown` without systemd) |
| `daemon`, `auth`, `config` | Reachability with version and health, the IPC auth mode, the configuration file's validity |
| `compositor`, `session_type` | The compositor name with its `source` (the daemon's detection, or this process's environment when the daemon is unreachable) and `wayland` or `x11` |
| `tier` | The hardware tier the benchmarks use (`gpu` or `cpu`) with the reason: the Vulkan device, an engine with a GPU path that fell back (the CPU-only diarization engine never counts), or `DETTIVO_FORCE_CPU=1` ([docs/qa.md](qa.md)) |
| `portals` | The `org.freedesktop.portal.*` interfaces the desktop portal exposes on the session bus, with `global_shortcuts` and `remote_desktop` called out |
| `platform`, `insertion`, `hotkeys`, `snippet` | The `system.capabilities.platform` block; every insertion backend with its availability and reason and the focused target; the hotkey backend with the portal and evdev availability; whether the compositor snippet is written and sourced |
| `audio` | PipeWire, the default source, the devices and the pinned one |
| `engines` | One row per engine binary: found where, running, the model, the backend with the engine's reason, crashes, degraded, and `memory_bytes` (the process's resident set while it runs) |
| `models` | The model directory, the catalogue version, the selected model, and readiness per provider (`providers[]`: total, ready, quarantined, selected) |
| `llm` | The selected provider and the provider order Enhanced walks with each one's availability, the local model's readiness with the download hint, and the engine's process (`llm.engine.status`, with the device memory it holds) |
| `history`, `osd` | The history database (path, size, item count, last migration); the pill's host and state, reported, never judged |
| `rest`, `mcp` | Whether the daemon hosts the REST listener with its bind and port, where the token comes from and whether one is required; the MCP server this CLI carries with the tools and resource templates it would offer |
| `omarchy` | On Omarchy, whether the shell runs, the plugin is installed and enabled and the panel holds its bus name ([docs/omarchy.md](omarchy.md)) |
| `environment` | Which `DETTIVO_*` overrides are set |

`dettivo insert` exits 1 when the insertion `failed` (the reason is on standard error) and 1 with `CONFLICT` when a guard mismatched the focused window; `--text -` reads the text from standard input so a compositor binding can pipe it.
