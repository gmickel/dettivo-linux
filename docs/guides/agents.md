# Agent guide

A skill, a script or an MCP client written against Dettivo on macOS drives Dettivo on Linux with the same method names, tool names, resource URIs, error codes and exit codes; this page is the one an agent author reads to find the surface they need (the `dettivo` command, MCP in four hosts, the REST shim) and the two documents that say where Linux differs and how a client finds out at run time. Every section links the topic page that holds the details.

## The command line

`dettivo` maps one verb to one contract method, prints human output by default and JSON with `--json`, says nothing on success with `--quiet` so a compositor binding can call it, and exits with the contract's codes. `dettivo call <method> [params]` reaches any method that has no verb yet. The tree below is generated from the binary by `dettivo docs cli-tree` and held to it by `just docs`; a `*` marks a Linux addition, each recorded in the [CLI additions](../api/linux-deltas.md#cli-additions) table of the delta register. The macOS nouns `transcript` and `meeting` are `history` and `meetings` here.

<!-- cli-tree:begin -->
```text
dettivo  Talk to the Dettivo daemon
├─ status  system.* methods
│  ├─ ping  system.ping
│  ├─ health  system.health
│  ├─ version  system.version
│  └─ capabilities  system.capabilities
├─ config*  Read and write config.toml through the daemon
│  ├─ get* [key]  Print one key, or every key, with its effective value and source
│  ├─ set* <key> <value>  Write one key; the file keeps its comments and ordering
│  ├─ unset* <key>  Remove one key so its default applies
│  ├─ path*  Print every path the daemon resolved
│  ├─ edit*  Open config.toml in $VISUAL or $EDITOR, then validate it
│  ├─ validate*  Validate the file on disk, naming the key and line of a problem
│  ├─ print-default*  Print the fully commented default file
│  └─ keys*  List every key with its section, type, default and meaning
├─ dictation  dictation.* methods: start, stop, cancel, status, toggle, reinsert-last
│  ├─ start --language --mode --expected-target-bundle-id --expected-target-pid  dictation.start
│  ├─ stop  dictation.stop: finish the take, transcribe and insert
│  ├─ cancel  dictation.cancel: drop the session
│  ├─ status  dictation.status
│  ├─ toggle*  Start when idle, stop when a session runs (Linux addition)
│  └─ reinsert-last*  dictation.reinsert_last: insert the last transcript again (Linux addition)
├─ events* --follow --topic --count  Follow the event stream (events.subscribe + events.notify), one JSON line per event
├─ speech*  speech.* methods
│  ├─ engines*  speech.engines: every engine binary, found or not, running or not
│  ├─ providers*  speech.providers.list: providers, their models and readiness
│  ├─ selection*  speech.selection.get / set: the dictation and meeting model
│  │  ├─ get*  Print the selection in force
│  │  └─ set* --provider --model --meeting-model --parakeet-model  Change the provider, the model or the meeting model
│  ├─ download* --model --provider --wait  speech.models.download: fetch and verify a catalogue model
│  ├─ cancel* --model --provider  speech.models.cancel: stop a download, keeping the partial file
│  ├─ delete* --model --provider --force  speech.models.delete: remove a model from disk
│  └─ status* --provider --follow  speech.models.status: every model's readiness
├─ insert --mode --text --kind --id --expected-target-bundle-id --expected-target-pid  Insert text into the focused window (insert.perform); `undo` and `target` are Linux additions
│  ├─ undo*  Take the last insertion back (insert.undo, within the undo window)
│  └─ target*  Show the focused window and every backend's availability (insert.target)
├─ history  transcripts.* methods: list, get, latest, search, delete, rerun, export, import
│  ├─ list --limit --app --since  transcripts.list: the newest items first
│  ├─ get <id> --words  transcripts.get: the final text of one item, or its segments and words
│  ├─ cancel* <id>  transcripts.cancel: stop the import or re-run job on an item between chunks
│  ├─ latest  transcripts.latest, then its text
│  ├─ search <query> --limit  transcripts.search: word-start matches over every text field
│  ├─ delete* <id>  transcripts.delete: the row and its artifacts per `[history] artifacts`
│  ├─ rerun* <id> --model --provider --mode --no-wait  transcripts.rerun: transcribe the retained audio again
│  ├─ export --scope --format --out --id --from --to  transcripts.export through a download transfer into a file
│  └─ import <file> --language --mode --provider --model --no-wait  Upload a file through transfer.* and transcripts.import it, following the job's progress
├─ meetings  meetings.* methods: start, stop, cancel, status, list, get, segments, search, notes, analyze, analysis, export, delete, recover, discard, disclosure
│  ├─ start --title --language --no-system-audio --acknowledge-disclosure  meetings.start: record the microphone and the default sink's monitor
│  ├─ stop <id>  meetings.stop: close the takes (answers stopping, then stopped)
│  ├─ cancel <id>  meetings.cancel: drop the meeting and its takes
│  ├─ status <id>  meetings.status: the state, the capture facts and the recoverable meetings
│  ├─ list --limit  meetings.list: the newest meetings first
│  ├─ get <id>  meetings.get: one meeting with its transcript and segments
│  ├─ segments* <id> --since --follow  The transcript so far as lines: side, span on the meeting clock, text, provisional lines marked ~; --since prints only what is new, --follow streams
│  ├─ search <query> --limit  meetings.search: word-start matches over the title, texts and summary
│  ├─ notes*  meetings.notes.get and meetings.notes.set: the notes, Markdown
│  │  ├─ get* <id>  Print the notes
│  │  └─ set* <id> [text] --file --stdin --source  Store the notes: the text, --file PATH or --stdin
│  ├─ analyze* <id> --force  meetings.analyze: summary, decisions and action items from the language model
│  ├─ analysis* <id>  meetings.analysis.get: the analysis and where it stands
│  ├─ export <id> --format --out --raw  transcripts.export through a download transfer into a file: txt, md, srt, vtt or json
│  ├─ delete <id> --policy  meetings.delete: remove the meeting per the artifact policy
│  ├─ recover* <id>  meetings.recover: retry a partial, failed, cancelled or stopped meeting with retained audio
│  ├─ discard* <id>  meetings.discard: remove a meeting a daemon restart left partial
│  ├─ disclosure* --acknowledge --copy  meetings.disclosure.get, acknowledge it with --acknowledge, or copy the message with --copy
│  ├─ diarize* <id> --speakers  meetings.diarize: run (or re-run) the speaker pass on a completed meeting
│  └─ speakers* [id]  meetings.speakers.list for a meeting id; `speakers rename <id> <speaker-id> <name>` and `speakers suggest` too
│     ├─ rename* <id> <speaker-id> <name>  meetings.speakers.rename: name a speaker across the meeting (an empty name restores the label)
│     └─ suggest* --prefix --limit  meetings.speakers.suggest: the names used before, most recent first
├─ audio*  audio.devices: the capturable nodes, the default source and the default sink
│  └─ devices*  audio.devices: the capturable nodes, the default source and the default sink
├─ polish  polish.* methods: try the layers, and edit the rules, presets and app profiles
│  ├─ test <text> --preset --style --bundle-id --mode --rule  Run a sample through the layers and print what each one made of it
│  ├─ rules  The custom rules the Enhanced rewrite carries
│  │  ├─ list  polish.rules.list
│  │  ├─ add <name> <content> --disabled  polish.rules.create
│  │  ├─ set <rule-id> <content> --disabled  polish.rules.update
│  │  └─ remove <rule-id>  polish.rules.delete
│  ├─ presets  polish.presets.list: the presets a profile or a test may name
│  └─ apps  The per-app presets
│     ├─ list  polish.apps.list
│     └─ set <bundle-id> <preset>  polish.apps.set
├─ llm*  llm.* methods (Linux additions): the providers behind Enhanced and the endpoint trust gate
│  ├─ providers*  llm.providers.list: which providers answer right now
│  ├─ trust* <url>  llm.endpoints.trust: allow an endpoint that is not on this machine
│  ├─ endpoints*  llm.endpoints.list: the endpoints in force and which are local
│  ├─ test* <text> --preset  Run a sample through the Enhanced pass (polish.test with mode=enhanced)
│  ├─ download* --model --wait  llm.models.download: fetch and verify a catalogue language model
│  ├─ cancel* --model  llm.models.cancel: stop a download, keeping the partial file
│  ├─ delete* --model --force  llm.models.delete: remove a language model from disk
│  ├─ status*  llm.models.status: every language model's readiness
│  ├─ engine*  llm.engine.status: the local engine process, its backend and memory
│  └─ experiment*  The sideloaded polish fine-tune: status, use, clear
│     ├─ status*  The sideload in force: its manifest, readiness and form
│     ├─ use* [name]  Run Enhanced on the fine-tune a manifest names (sets `[llm] polish_experiment`)
│     └─ clear*  Back to the catalogue model (clears `[llm] polish_experiment`)
├─ app*  Raise the desktop app, open a route in it, or read its status, without the daemon
│  ├─ open* <route> --id  Open a route in the running window, or launch the app on it
│  └─ status*  The running app's route, window and daemon link
├─ osd*  Show, hide or inspect the recording pill (dettivo-osd), without the daemon
│  ├─ show* <state> --title --hint --engine --words --target --reason --action --level  Show one pill state (listening, transcribing, enhancing, inserted, copied, error)
│  ├─ hide*  Hide the pill
│  ├─ status*  Which host runs the pill, where it sits and what it shows; the disabled notice when no process runs
│  └─ host-panel*  Hold the Omarchy panel's bus name while the plugin hosts the pill (exits 0 at once unless `[omarchy] osd = "panel"`)
├─ doctor*  Report service, socket, config, engine, platform, insertion, hotkey, history and pill facts
├─ setup* <compositor> --stdout --check --no-plugin --git  Write the compositor binding snippet (hyprland, sway, niri) with the `[hotkeys]` chords; `omarchy` does the whole install
├─ hotkeys*  hotkeys.status: the daemon-side backend, the portal and evdev availability, the bound actions
│  └─ status*  hotkeys.status: the backend in force, portal and evdev availability, bound actions
├─ mcp  The MCP server over stdio, the host configuration and a check (docs/mcp.md)
│  ├─ serve  Serve MCP over standard input and output (what a host's config runs)
│  ├─ config --host --name --command --write --hardened  Print a host's server entry, or merge it into the host's file with --write
│  └─ check*  Initialise against the daemon and report the tool count and the framing
├─ rest*  The loopback REST shim: host it in this process, report the listener, name the token source (docs/rest.md)
│  ├─ serve* --port --bind  Host the shim in this process until interrupted
│  ├─ status*  Report the listener: hosted by the daemon or a process, answering, authorised
│  └─ token*  Print where the REST token comes from (never the token itself)
├─ call* <method> [params]  Call any contract method with JSON params
├─ docs*  Generate documentation from this binary: `cli-tree` prints the command tree with every Linux addition marked (docs/guides/agents.md)
│  └─ cli-tree* --register  Print the command tree with every Linux addition marked `*`
└─ completions* <shell>  Print the shell completion script for bash, zsh or fish (the package installs all three)

global: --socket --token --token-file --timeout-ms --json --quiet
env:    DETTIVO_IPC_SOCKET DETTIVO_IPC_TOKEN
exit:   0 ok, 1 runtime, 2 daemon unavailable, 3 permission, 4 args, 5 timeout
*       a Linux addition recorded in docs/api/linux-deltas.md
```
<!-- cli-tree:end -->

The daemon is reached over `$XDG_RUNTIME_DIR/dettivo/dettivo.sock` (`DETTIVO_IPC_SOCKET` or `--socket` overrides it) and, under `[ipc] auth_mode = "peer_token"`, with the token from `DETTIVO_IPC_TOKEN`, the Secret Service item or the `0600` token file ([docs/daemon.md](../daemon.md#the-socket)). `dettivo events --follow --json` streams every event as one JSON line, which is how a script waits for a dictation to complete ([docs/dictation.md](../dictation.md#the-event-stream)).

## MCP

```
dettivo mcp config --host claude-code            # prints the `claude mcp add` line
dettivo mcp config --host claude-desktop --write # merges into the host's file
dettivo mcp config --host cursor --write
dettivo mcp config --host codex --write
dettivo mcp check                                # the tool count and the framing against the daemon
```

The server speaks JSON-RPC over stdio in both framings, exposes the nineteen macOS tools (`get_status`, `list_transcripts`, `search_transcripts`, `insert_transcript`, `start_dictation`, the meeting tools and the rest) plus the Linux `get_meeting_segments` and the `status://`, `transcript://`, `meeting://` and `transcripts://` resources, and bounds every list and message the way macOS does, with `_truncated` on a shortened list ([docs/mcp.md](../mcp.md)). `get_status` merges `system.capabilities` under `capabilities`, so one call tells an agent which flags are on.

## A live meeting copilot

An agent can sit beside you in a call and read the conversation as it happens: what you were asked, what is still open, what to say next. It needs four steps, and it never needs a subscription that was open from the start.

1. Find the meeting that records now. `dettivo --json status health` answers `recording_state = meeting`, and `dettivo --json meetings list --limit 1` answers the newest meeting with `status = recording`.
2. Read the backlog once. `dettivo --json meetings segments <id>` answers everything said so far, including what was said before the agent attached. Keep its `cursor`.
3. Every minute or two, read what is new. `dettivo --json meetings segments <id> --since <cursor>` answers only the finals after the cursor, plus the provisional tail, and the next cursor. Alert early from `provisional` if you like, and build the running summary from `segments` alone. `source` (`you` or `remote`) and `source_type` (`microphone` or `system`) mark your own lines, so the agent can keep them out of its alerts. An agent that prefers a stream runs `dettivo --json meetings segments <id> --follow` instead, which prints the backlog and then every event once.
4. Watch `transcript` and `reset`. The live transcript answers until the finalisation has stored the meeting, and `status` moves through `stopping`, `stopped` and `transcribing` meanwhile. When `transcript` turns `stored` the answer carries `reset = true` and the whole finalised transcript, which replaces the live copy. The meeting is done when `status = completed`.

The agent only reads. It never stops, cancels or deletes the meeting, and a read never slows the capture or writes a file (ADR 0071). Over MCP the same loop is `get_meeting_segments` with `since`; over REST it is `GET /v1/meetings/segments?meeting_id=<id>&since=<cursor>`. The cursor rules are in [docs/meetings.md](../meetings.md#the-transcript-so-far).

## REST

Anything that speaks HTTP sends `GET` or `POST` to `http://127.0.0.1:45831/v1/{namespace}/{method}` with the shared token and gets the method's result back; raw audio posted to `/v1/transcripts/import/stream` becomes a history item and `/v1/transcripts/export/stream` streams an export ([docs/rest.md](../rest.md)). The shim is off by default, binds loopback only, and is hosted by the daemon under `[rest] enabled = true` or by `dettivo rest serve` on demand.

## Capability flags

`system.capabilities` is the contract's answer to "what does this daemon do", and a client branches on its flags, never on the platform: `platform` names the OS, the compositor, the session, the insertion backend, the GPU and the hardware `tier`; `speech.methods`, `insert.methods`, `config.methods`, `history.methods`, `meetings.methods` and `llm.methods` list the Linux-adopted methods; `formats.meeting_export` lists the export formats; `polish.modes` the dictation modes; `rest` the listener. Every addition is one row of the [additions table](../api/linux-deltas.md#additions-linux-makes) with the flag that signals it, and `crates/dettivo-proto/fixtures/` holds one fixture per method that the contract replay checks against a live daemon ([docs/guides/qa.md](qa.md)).

## The delta register

[docs/api/linux-deltas.md](../api/linux-deltas.md) is the one place that says where Linux differs from the copied macOS contract ([docs/api/dettivo-ipc-v1.md](../api/dettivo-ipc-v1.md), [docs/api/dettivo-rest-v1.md](../api/dettivo-rest-v1.md), hashes pinned in `CONTRACT_PINS.txt`): every method, tool, resource, export format and event topic has one disposition (`implemented`, `covered-elsewhere`, `deferred`, `blocked`), the additions are listed with their flags, the behavioural differences with their reasons, and `just lint-deltas` fails when an item has no row. A method the daemon has not implemented answers `NOT_IMPLEMENTED` and its flag is `false`, so a client written for the macOS extras degrades by reading the flag ([ADR 0008](../adr/0008-contract-parity.md)).

## Exit codes and errors

| Exit | Meaning |
|---|---|
| 0 | Success. |
| 1 | Runtime failure; the error object names the `app_code` and the contract's `details`. |
| 2 | The daemon or its socket is unavailable. |
| 3 | Permission denied by the peer check or the token. |
| 4 | Invalid command or arguments. |
| 5 | Timeout (`--timeout-ms`, default 5000) or an interrupted operation. |

With `--json`, a daemon failure prints its unchanged JSON-RPC error object on standard output. A locally detected failure prints `{ "error": { "code": -32603, "message": "...", "data": { "origin": "cli", "exit_code": 4 } } }`, with the actual exit code. Scripts branch on `error.data.origin` for local errors and `error.data.app_code` for daemon errors. Commands that already return a structured diagnostic report retain that single report on failure. `--quiet` suppresses stdout while stderr retains the failure message, and `mcp serve` reserves stdout for framed MCP messages (ADR 0055).
