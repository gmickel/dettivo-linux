# Dettivo Local REST Shim v1

Status: Draft v1 (`fn-2.5`)  
Source of truth: `docs/api/dettivo-ipc-v1.md`

## 1. Purpose
Local HTTP adapter over IPC JSON-RPC contract for tools that cannot use Unix socket JSON-RPC or MCP.

- Transport: HTTP/1.1
- Bind: loopback only (`127.0.0.1`)
- Default: disabled
- Auth: required shared token on every request
- Semantics: method/result/error parity with IPC contract

## 2. Enablement

In app settings:
- `Settings -> REST API -> Enable local REST shim`

Runtime defaults:
- `api.rest.enabled=false`
- `api.rest.port=45831`

Base URL:
- `http://127.0.0.1:<port>`

## 3. Authentication
Token required for all endpoints.

Request header (either form):
- `Authorization: Bearer <token>`
- `X-Dettivo-Token: <token>`

Token lookup order matches IPC hardened mode:
1. `DETTIVO_IPC_TOKEN` env
2. Keychain (`service=com.dettivo.ipc`, `account=local-shared-token`)
3. `~/Library/Application Support/Dettivo/ipc.token` (must be mode `0600`)

Missing/invalid token response:
- HTTP `401`
- `error.data.app_code = "UNAUTHORIZED_CLIENT"`

## 4. Endpoint Mapping
Generic mapping:
- Path: `/v1/{namespace}/{method}[/{submethod}...]`
- Method name: slash segments joined by dots
- Example: `GET /v1/system/health` -> `system.health`

Supported verbs:
- `GET` or `POST` for contract method endpoints
- `POST` for `transcripts/import/stream`
- `GET` for `transcripts/export/stream`

Request params:
- Query params are parsed into IPC params.
- JSON body object (if present) overlays query params.

## 5. Streaming Endpoints
### 5.1 Import stream
Dictation:
`POST /v1/transcripts/import/stream?target_kind=dictation&mode=raw&language=en&filename=audio.wav`

Meeting:
`POST /v1/transcripts/import/stream?target_kind=meeting&filename=call.m4a`

Body:
- raw audio bytes

Behavior:
- uses transfer pipeline internally (`transfer.begin/chunk/commit`)
- calls `transcripts.import`
- preserves the `filename` query value, after sanitizing path separators/control characters, for extension detection and default imported meeting titles
- if `filename` is missing or unsupported, content type is used as a fallback extension
- REST loopback requests are bounded to 50 MB. Use the app file picker or CLI import path for long meeting recordings.

Response:
- same JSON as `transcripts.import`

### 5.2 Export stream
`GET /v1/transcripts/export/stream?kind=meeting&id=<uuid>&format=md`

Behavior:
- uses transfer pipeline internally (`transfer.begin`, `transcripts.export`, `transfer.pull`)

Response:
- binary transcript payload
- `Content-Type` from export result
- `Content-Disposition: attachment; filename="<filename>"`

## 6. Error Schema
REST error body mirrors IPC error object:

```json
{
  "error": {
    "code": -32010,
    "message": "Invalid params",
    "data": {
      "app_code": "INVALID_PARAMS",
      "retryable": false,
      "details": {}
    }
  }
}
```

HTTP status mapping:
- `INVALID_PARAMS` -> `400`
- `UNAUTHORIZED_CLIENT` -> `401`
- `NOT_FOUND` -> `404`
- `CONFLICT` -> `409`
- `RATE_LIMITED_LOCAL` -> `429`
- `NOT_IMPLEMENTED` -> `501`
- `APP_NOT_RUNNING` -> `503`
- fallback -> `500`

## 7. Examples
Health:

```bash
curl -s \
  -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" \
  http://127.0.0.1:45831/v1/system/health
```

Meeting start:

```bash
curl -s -X POST \
  -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title":"Weekly Sync","capture":{"system_audio":true,"microphone":true}}' \
  http://127.0.0.1:45831/v1/meetings/start
```

Export transcript file:

```bash
curl -L \
  -H "Authorization: Bearer $DETTIVO_IPC_TOKEN" \
  "http://127.0.0.1:45831/v1/transcripts/export/stream?kind=dictation&id=<uuid>&format=txt" \
  -o transcript.txt
```

## 8. Security Notes
- Loopback only by bind policy and request host validation.
- No remote access by design.
- No multi-user auth model; per-user local token only.
- Keep token out of shell history and logs.
