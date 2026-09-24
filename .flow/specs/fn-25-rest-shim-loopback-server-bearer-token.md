# REST shim: loopback server, bearer token, streaming import and export, the VS Code check

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-30 row: "`rest-shim-loopback-token-streaming-vscode-check` | phase 5 | depends on S-11, S-15 | FR-S7; VS Code extension verification | REST fixtures; extension smoke on Thor"
> masterplan FR-S7: "The REST shim binds `127.0.0.1` on port 45831 by default, requires `Authorization: Bearer` or `X-Dettivo-Token`, routes `/v1/{namespace}/{method}` for `GET` and `POST` with query parameters overlaid by a JSON body, special-cases `POST /v1/transcripts/import/stream` (raw audio body, 50 MB cap) and `GET /v1/transcripts/export/stream`, maps `app_code` to HTTP status as macOS does (400, 401, 404, 409, 429, 501, 503, else 500), and is off by default."
> masterplan FR-S2: "Error taxonomy, numeric codes and retryability match the contract"
> masterplan FR-S1: "optional shared token resolved in the macOS order (`DETTIVO_IPC_TOKEN`, Secret Service item `com.dettivo.ipc` / `local-shared-token`, then `$XDG_CONFIG_HOME/dettivo/ipc.token` mode `0600`)"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Anything that speaks HTTP reaches Dettivo the way it reaches the macOS app. `dettivo rest serve` (and `[rest] enabled = true` inside the daemon) binds a loopback server on port 45831, requires the shared token as a bearer or `X-Dettivo-Token` header, routes `/v1/{namespace}/{method}` for GET and POST onto the daemon's IPC methods with query parameters overlaid by the JSON body, streams raw audio bodies into `transcripts.import` and export bytes out of `transcripts.export`, maps the contract's `app_code` to the macOS HTTP statuses, and is off by default. REST fixtures per route prove the mapping, and the macOS VS Code extension's request shapes are the smoke check that parity holds. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `crates/dettivo-rest` (depends on `dettivo-proto`, `dettivo-core`; a small hand-rolled HTTP/1.1 server over `tokio` with `httparse` and no framework, the way the router and the MCP layer are hand-rolled): `server` (bind `127.0.0.1:<port>`, keep-alive, request size cap, per-request timeout), `auth` (the token resolved in the macOS order; missing or wrong answers 401 with the contract's `UNAUTHORIZED_CLIENT` body; `[rest] require_token = true` always on since the bind is loopback but other users on the machine exist), `routes` (`/v1/{namespace}/{method}` to the IPC method `{namespace}.{method}` for GET with query parameters and POST with a JSON body overlaid on the query; the reserved shape for `NOT_IMPLEMENTED`), `stream` (`POST /v1/transcripts/import/stream` reads the raw body up to 50 MB into `transfer.begin|chunk|commit` and `transcripts.import`, `GET /v1/transcripts/export/stream?…` runs `transcripts.export` and streams `transfer.pull` chunks as the response body with the content type and filename headers), `status` (the `app_code` to HTTP status map: `INVALID_PARAMS` 400, `UNAUTHORIZED_CLIENT` 401, `NOT_FOUND` 404, `CONFLICT` 409, `RATE_LIMITED_LOCAL` 429, `NOT_IMPLEMENTED` 501, `APP_NOT_RUNNING` 503, else 500, with the JSON-RPC error object as the body), `client` (a blocking Unix socket client to the daemon per request, or the daemon's in-process router when embedded). [paraphrase]
- Hosting: `dettivo rest serve [--port]` runs it as a process (the CLI links the crate) for users who want it on demand, and `[rest] enabled = true` makes the daemon host it in-process on start (`crates/dettivod/src/rest.rs` spawning the server on the runtime) so a service unit is enough; `[rest] port = 45831`, `bind = "127.0.0.1"` (loopback only; any other value is refused at validation with the reason). [inferred]
- Fixtures: `crates/dettivo-rest/fixtures/<route>.json` with request (method, path, query, headers, body) and expected status, headers and body per route, including the streaming routes with a small WAV and the error mappings; the harness runs them against a live seeded daemon; `dettivo-qa rest` runs the same harness in the contract replay and CI. [paraphrase]
- VS Code check: the macOS VS Code extension's request shapes (the paths, headers and bodies it sends: status, latest transcript, insert, start and stop dictation, export stream) are captured as fixtures from the extension source in the macOS repository and replayed by the harness, so the extension works unchanged against Linux; the extension smoke on this desktop is a documented manual step with the extension pointed at the Linux port. [paraphrase]
- Capabilities and doctor: `system.capabilities.rest` (`enabled`, `port`, `bind`), a doctor row naming the listener and its token requirement. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Routes: `GET|POST /v1/{namespace}/{method}` for every implemented IPC method (query parameters as strings coerced by the method's parameter types; the JSON body overlays), `POST /v1/transcripts/import/stream` (`Content-Type` selects the audio type, `X-Dettivo-Filename` optional, 50 MB cap answering 413 with the reason), `GET /v1/transcripts/export/stream` (query `format`, `id`/`scope`, streams the export). [paraphrase]
- Headers: `Authorization: Bearer <token>` or `X-Dettivo-Token: <token>`; responses `Content-Type: application/json` for methods, the export's type for the stream. [paraphrase]
- CLI: `dettivo rest serve [--port <n>]`, `dettivo rest status`, `dettivo rest token` (prints where the token comes from, never the token). [inferred]
- Config: `[rest] enabled = false`, `port = 45831`, `bind = "127.0.0.1"`, `max_body_bytes = 52428800`, `request_timeout_ms = 30000`. [inferred]
- Events over REST: not in scope; `/v1/events/subscribe` answers 501 with guidance. [inferred]

## Edge Cases & Constraints

- A bind outside loopback in config: validation refuses it naming the key; the server never starts on it. [paraphrase]
- No token resolvable: the server refuses to start with the three sources named. [inferred]
- Body over the cap: 413 before reading the rest; the connection closes. [paraphrase]
- The daemon unavailable: 503 with `APP_NOT_RUNNING` and the systemd hint. [paraphrase]
- Two servers (CLI and embedded) on one port: the second fails naming the first. [inferred]

## Acceptance Criteria

- **R1:** The route fixtures pass against a live seeded daemon for every implemented namespace (system, config, speech, dictation, transcripts, hotkeys, polish, insert), GET with query parameters and POST with a body overlay both, the `NOT_IMPLEMENTED` reserved shape for the rest, and the `app_code` to status map for every error fixture. Errors: an unknown route is 404 with the reserved shape. [paraphrase]
- **R2:** Auth: no header, a wrong token and each of the three token sources are tested; the bearer and the `X-Dettivo-Token` forms both work; a non-loopback bind is refused at validation. Errors: as stated. [paraphrase]
- **R3:** Streaming: `POST /v1/transcripts/import/stream` with a WAV body imports an item (mock engine) and answers the import result; a body over the cap answers 413; `GET /v1/transcripts/export/stream` streams the export with the right content type and matches the store's export goldens. Errors: as stated. [paraphrase]
- **R4:** The daemon hosts the server in-process when `[rest] enabled = true` and `dettivo rest serve` hosts it as a process; `dettivo rest status` and `system.capabilities.rest` report the listener; the second server on the port fails naming the first. Errors: as stated. [paraphrase]
- **R5:** The VS Code extension's request shapes, captured from the macOS extension source, replay through the harness unchanged, and `dettivo-qa rest` runs the harness in the contract replay and the CI contract job. Errors: as stated. [paraphrase]
- **R6:** `docs/rest.md` explains the routes, auth, streaming, statuses and the VS Code setup; the config keys are documented and printed by `config print-default`; an ADR records the hand-rolled server and the status map. Errors: as stated. [paraphrase]

## Boundaries

- No TLS, no non-loopback binding, no events over REST. [paraphrase]
- No new IPC methods. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The macOS VS Code extension and every HTTP-speaking tool reach Dettivo through this shim without changes; parity is the whole point. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** the same routes, headers and status map as macOS, proven by fixtures and the extension's own request shapes. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
