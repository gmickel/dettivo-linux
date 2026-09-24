# REST

Anything that speaks HTTP reaches Dettivo the way it reaches the macOS app: `curl`, a shell script, an editor extension or a browser tool sends `GET` or `POST` to `http://127.0.0.1:45831/v1/{namespace}/{method}` with the shared token and gets the contract method's result back as JSON. Raw audio posted to `/v1/transcripts/import/stream` becomes a history item; `/v1/transcripts/export/stream` hands a transcript back as a file. The routes, the headers, the status map and the error body are the ones `docs/api/dettivo-rest-v1.md` fixes, so a call written against macOS works here unchanged (ADR 0028, ADR 0008).

## Turning it on

The shim is off by default and binds loopback only. Two ways to run it:

```
dettivo config set rest.enabled true     # the daemon hosts it from its next start
dettivo rest serve [--port 45831]        # this process hosts it until interrupted
```

`[rest] enabled = true` in `config.toml` makes `dettivod` start the listener beside its socket, so a systemd user service is all that is needed; the setting is read at daemon start. `dettivo rest serve` runs the same server in a process over the daemon's Unix socket for a user who wants it on demand, taking `--port` and `--bind` over the `[rest]` keys. A second server on the same port fails naming the port and the likely owner; `dettivo rest status` shows which one is listening, whether it answers, and whether the resolved token is accepted:

```
listener  http://127.0.0.1:45831 hosted by the daemon ([rest] enabled)
auth      answers 200 with the resolved token
token     from environment
```

`system.capabilities.rest` (`enabled`, `port`, `bind`) reports the daemon's listener to any client, and `dettivo doctor` prints the same row.

`dettivo rest status` is a readiness check. A `200` response exits `0`, `401` exits `3`, `503` or a refused connection exits `2`, and other HTTP failures exit `1`. The report preserves the actual HTTP status and error body; `--json` keeps them in structured form. A listening shim whose daemon is unavailable reports `503`, so scripts can distinguish it from a refused token (ADR 0050).

## The token

Every request carries the shared token, in either header form:

```
Authorization: Bearer <token>
X-Dettivo-Token: <token>
```

The token is resolved in the macOS order with the Keychain replaced by the freedesktop Secret Service: `DETTIVO_IPC_TOKEN`, then the Secret Service item (`secret-tool store --label dettivo service dettivo key ipc-token`), then `$XDG_CONFIG_HOME/dettivo/ipc.token` with mode `0600` (`[ipc] token_file` moves it). It is the same token the daemon's `peer_token` mode checks, so one setting serves both. `dettivo rest token` prints where the token comes from and never the token; `dettivo rest serve` also takes `--token` and `--token-file`. With no token in any source the listener does not start and the three sources are named. A missing or wrong token answers `401` with the contract's `UNAUTHORIZED_CLIENT` body (`Missing auth token`, `Invalid auth token`), and so does a `Host` header that is not loopback.

## Routes

The process-hosted shim forwards the resolved HTTP token to the daemon for both settings lookup and method calls, including when Secret Service or the configured token file supplied it. Malformed non-ASCII authorization prefixes receive `401` and leave the server available for the next request.

`/v1/{namespace}/{method}` maps to the contract method with the slashes turned into dots: `GET /v1/system/health` is `system.health`, `POST /v1/polish/rules/create` is `polish.rules.create`. `GET` and `POST` are both accepted for every method. Parameters come from the query string, coerced the way the macOS shim coerces them (`true` and `false` to booleans, an integer to a number, `a,b` to a list of strings, anything else a string), and a JSON object body overlays them key by key. A parameter the contract types as a list (`kinds` on `transcripts.list` and `transcripts.search`) is written as a comma list in the query or as an array in the body:

```bash
curl -s -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" \
  "http://127.0.0.1:45831/v1/transcripts/list?kinds=dictation&limit=5"      # "dictation" alone is a string
curl -s -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" -H "Content-Type: application/json" \
  -d '{"kinds":["dictation"],"limit":5}' http://127.0.0.1:45831/v1/transcripts/list
```

The daemon validates the parameters against the contract exactly as it does on the socket, so the answer to a bad shape is the same `INVALID_PARAMS` as `400`. A path outside `/v1/` or a method the contract does not name is `404`; a reserved method is the daemon's `NOT_IMPLEMENTED` as `501`; `PUT` and the other verbs are `400`. Events are not served over REST: `/v1/events/subscribe` answers `501` and names `dettivo events --follow` and MCP, which stream them.

## Streaming

`POST /v1/transcripts/import/stream` takes the raw audio bytes as the body. `Content-Type` names the audio type (`audio/wav`, `audio/mpeg`, `audio/mp4`, `audio/flac`, `audio/ogg`, `audio/x-caf`, `audio/aiff`, `application/zip` for a Dettivo archive); the query carries `transcripts.import`'s parameters, and the ones absent take their defaults: `target_kind=dictation`, `mode=raw`, `language=auto`, and the filename from `filename=`, then the `X-Dettivo-Filename` header, then the content type (`audio.wav`). The shim runs `transfer.begin`, `transfer.chunk` and `transfer.commit` and answers with the `transcripts.import` result; a failure cancels the transfer with the macOS reason (`stream_chunk_failed`, `stream_commit_failed`, `import_failed`). The body is capped at `[rest] max_body_bytes` (50 MiB); a larger `Content-Length` is answered `413` before the body is read and the connection closes. `dettivo history import` has no cap.

```bash
curl -s -X POST -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" -H "Content-Type: audio/wav" \
  --data-binary @call.wav \
  "http://127.0.0.1:45831/v1/transcripts/import/stream?language=en&filename=call.wav"
```

`GET /v1/transcripts/export/stream` runs `transcripts.export`, verifies its chunks and final acknowledgement into an anonymous temporary file, then streams that file with a bounded buffer. Failed transfer completion returns the mapped error before HTTP success headers. `format` is required; `kind` defaults to `dictation`; `id` names one item, or `scope=all` and `scope=range&from=…&to=…` export a day or everything (the Linux additions `transcripts.export` carries). The response carries the export's `Content-Type` and `Content-Disposition: attachment; filename="<name>"`:

```bash
curl -s -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" \
  "http://127.0.0.1:45831/v1/transcripts/export/stream?scope=all&format=md" -o dictations.md
```

## Statuses and errors

Every error is the contract's JSON-RPC error object under `error`, and the HTTP status follows `app_code`:

| `app_code` | Status |
|---|---|
| `INVALID_PARAMS` | `400` (`413` for a body over the cap) |
| `UNAUTHORIZED_CLIENT` | `401` |
| `NOT_FOUND` | `404` |
| `CONFLICT` | `409` |
| `RATE_LIMITED_LOCAL` | `429` |
| `NOT_IMPLEMENTED` | `501` |
| `APP_NOT_RUNNING` | `503` |
| anything else | `500` |

`APP_NOT_RUNNING` (`-32017`) is what `dettivo rest serve` answers when the daemon is not reachable, with the socket path and `systemctl --user start dettivod.socket` in the message. A request that exceeds `[rest] request_timeout_ms` is `500`.

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `rest.enabled` | `false` | The daemon hosts the listener from its next start. |
| `rest.port` | `45831` | The TCP port; `0` takes an ephemeral one, reported in `system.capabilities.rest.port`. |
| `rest.bind` | `"127.0.0.1"` | A loopback address only (`127.0.0.1`, `::1`, `localhost`); any other value fails `config validate` naming `rest.bind`, and the file is refused (a start stops, an edit keeps the values in force) with no listener on the refused address. |
| `rest.max_body_bytes` | `52428800` | The largest request body; a longer one is `413`. |
| `rest.request_timeout_ms` | `30000` | The time one request may take end to end. |

## The fixtures and the VS Code check

`crates/dettivo-rest/fixtures/` holds one file per request: the routes of every implemented namespace with a `GET` query and a `POST` body overlay, both token headers, the missing and wrong token, every row of the status map, the `413`, the export of the seed compared with the store's goldens, the import of the fixture clip through the real engine, and the request shapes an IDE bridge sends under `vscode/`. `dettivo-qa rest` (`just qa-rest`) starts a seeded daemon with the shim on an ephemeral port and replays them; `dettivo-qa contract` carries the same section, and CI runs both. `crates/dettivo-rest/tests/harness.rs` runs the fixtures against the daemon-hosted shim and against a process-hosted one over the socket.

The macOS VS Code extension (`extensions/vscode-dettivo` in the macOS repository) syncs the editor context into a file and runs voice navigation commands; it does not call the REST shim itself, and its request shapes toward the app are the ones the macOS REST tests exercise (`RESTShimServiceTests.swift`): health, the latest transcript, one transcript, an insertion, the dictation state and an export stream. Those are the `vscode/` fixtures, and they pass unchanged here. The manual smoke on a desktop is to point the extension's host at this machine: install it into VS Code or Cursor, run `dettivo rest serve` (or enable `[rest]`), export `DETTIVO_IPC_TOKEN`, and replay the same calls with `curl` from the extension's shell; the fixtures are the record of what must answer.
