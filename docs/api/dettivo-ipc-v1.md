# Dettivo IPC API v1 Contract

Status: Draft v1 (normative for `fn-2.1`)  
Owners: Dettivo app team  
Consumers: CLI (`dettivo`), MCP server, optional REST shim

## Implementation status (fn-2.2)
- Implemented in-app via `DefaultIPCService` + `IPCSocketServer`.
- Lifecycle is app-scoped: server starts with `AppState` init and stops in `AppState.cleanup()`.
- Runtime config keys:
  - `api.ipc.authMode` (`peer` | `peer_token`)
  - `api.ipc.transferChunkMaxBytes`
  - `api.ipc.transferTimeoutSeconds`
- Hardened auth token lookup order:
  1. `DETTIVO_IPC_TOKEN` env
  2. Keychain (`com.dettivo.ipc` / `local-shared-token`)
  3. `~/Library/Application Support/Dettivo/ipc.token`
- Coverage: `DettivoTests/IPCServiceTests.swift` validates peer mode dispatch, peer+token auth, and subscription backpressure overflow behavior.

## Implementation status (fn-2.4 MCP adapter)
- Implemented in CLI binary as `dettivo mcp serve` (stdio MCP server process).
- `tools/list` + `tools/call` bridge to IPC methods only (no business logic duplication).
- `resources/list`, `resources/read`, and `resources/templates/list` support:
  - `status://current`
  - `transcript://{id}`
  - `meeting://{id}`
  - `transcripts://search/{query}?kinds=dictation,meeting&limit=10`
  - `transcripts://latest/{kind}`
- Host config helper: `dettivo mcp config --host claude-desktop|cursor|codex` (`--write` supported for Claude Desktop and Codex).
  - Codex writes to `$CODEX_HOME/config.toml` when `CODEX_HOME` is set, otherwise `~/.codex/config.toml`.
  - Default command path resolution prefers `~/.local/bin/dettivo`, then stable current executable/app-bundle paths, then standard app installs before falling back.
- Coverage: `DettivoCLITests/MCPServerTests.swift` validates MCP -> IPC read/write bridging and actionable unavailable guidance.

## Implementation status (fn-2.5 REST shim)
- Implemented in-app as optional loopback HTTP adapter over `DefaultIPCService`.
- Toggle: `api.rest.enabled` (default `false`), port via `api.rest.port` (default `45831`).
- Security: token required for all REST requests, same token source order as hardened IPC.
- Endpoint mapping: `/v1/{namespace}/{method...}` -> IPC method names (slash -> dot).
- Streaming endpoints:
  - `POST /v1/transcripts/import/stream`
  - `GET /v1/transcripts/export/stream`
- Coverage: `DettivoTests/RESTShimServiceTests.swift` validates auth, health/meeting/transcript route parity, and streaming import/export paths.
- Reference: `docs/api/dettivo-rest-v1.md`

## 1. Scope and Principles
Canonical local API for Dettivo automation.

- IPC JSON-RPC is the source of truth
- CLI/MCP/REST are adapters only (no divergent business logic)
- if behavior changes, IPC contract changes first

Scope covered in v1:
- dictation + meeting lifecycle control
- import audio bytes for transcription
- list/get/search/latest transcript/history access
- export (`txt|md|json|srt|vtt`) with scoped capability signaling
- insertion into focused app
- polish rules/presets/test/app mapping
- health/version/capabilities
- event subscriptions
- binary transfer framing

## 2. Transport and Security
## 2.1 IPC transport
- protocol: JSON-RPC 2.0
- transport: Unix domain socket
- socket path default (sandbox-safe): `~/Library/Containers/com.dettivo.app/Data/tmp/dettivo.sock`
- legacy fallback path (CLI only): `~/Library/Application Support/Dettivo/dettivo.sock`
- socket mode: `0600`

## 2.2 Auth model
No account auth. Local process auth.

### IPC default mode (compatibility mode)
- peer uid/pid verification
- socket permission checks
- no token required

### IPC hardened mode (optional)
- peer uid/pid verification
- shared token required
- token lookup:
  1. Keychain
  2. fallback token file (`0600`)

### REST mode
- loopback-only bind
- token always required
- same token source as hardened IPC mode

Capability signaling:
- `auth.ipc_mode = "peer" | "peer_token"`
- `auth.rest_token_required = true`

Note on PRD drift: PRD text saying local REST has "no auth" is superseded by this v1 contract.

## 3. IDs and Resource Model
Dettivo stores two primary history entities:
- dictation items
- meeting sessions

To avoid adapter drift, transcript APIs use a union reference.

### 3.1 ID formats
- all IDs are lowercase UUID strings
- dictation ID key: `dictation_id`
- meeting ID key: `meeting_id`

### 3.2 `TranscriptRef`
```json
{
  "kind": "dictation|meeting",
  "id": "<uuid>"
}
```

### 3.3 `HistoryItem`
```json
{
  "ref": { "kind": "meeting", "id": "0f8fad5b-d9cb-469f-a165-70867728950e" },
  "title": "Weekly sync",
  "started_at": "2026-02-13T16:00:00Z",
  "duration_seconds": 1800,
  "status": "idle|recording|transcribing|completed|failed|cancelled"
}
```

## 4. JSON-RPC Envelope
### 4.1 Request
```json
{
  "jsonrpc": "2.0",
  "id": "req-123",
  "method": "group.method",
  "params": {}
}
```

### 4.2 Success
```json
{
  "jsonrpc": "2.0",
  "id": "req-123",
  "result": {}
}
```

### 4.3 Error
```json
{
  "jsonrpc": "2.0",
  "id": "req-123",
  "error": {
    "code": -32001,
    "message": "Unauthorized client",
    "data": {
      "app_code": "UNAUTHORIZED_CLIENT",
      "retryable": false,
      "details": {
        "domain": "dettivo_error",
        "dettivo_error_code": 401,
        "dettivo_error_kind": "accessibilityPermissionDenied"
      }
    }
  }
}
```

`error.data.details` mapping to runtime errors is optional but stable when present.

## 5. Error Taxonomy
| App Code | Meaning | Retryable |
|---|---|---|
| `INVALID_PARAMS` | Bad/missing params | No |
| `UNAUTHORIZED_CLIENT` | Peer/token/security policy failed | No |
| `NOT_FOUND` | Resource not found | No |
| `CONFLICT` | Invalid current state | Sometimes |
| `NOT_IMPLEMENTED` | Reserved capability/method not enabled | No |
| `RATE_LIMITED_LOCAL` | Local backpressure | Yes |
| `INTERNAL_ERROR` | Unexpected server failure | Maybe |
| `APP_NOT_RUNNING` | Adapter-only connectivity failure | Yes |

`APP_NOT_RUNNING` is adapter-only (CLI/MCP/REST). IPC server itself does not emit it.

## 6. Version and Capabilities
## 6.1 `system.version` result
```json
{
  "api_version": "1.0.0",
  "app_version": "0.9.0",
  "build": "76e3dea"
}
```

## 6.2 `system.capabilities` result (minimum)
```json
{
  "auth": {
    "ipc_mode": "peer",
    "rest_token_required": true
  },
  "formats": {
    "dictation_export": ["txt", "md", "json"],
    "meeting_export": ["txt", "md", "srt", "vtt"]
  },
  "insert_modes": ["raw", "polish", "clipboard_only"],
  "transfers": {
    "chunk_max_bytes": 1048576,
    "max_inflight": 4,
    "timeout_seconds": 120
  },
  "automation": {
    "email_delivery": false,
    "jobs": false,
    "providers": false,
    "dictation_macros": true,
    "dictation_macro_audit": false
  },
  "knowledge": {
    "semantic_search": false,
    "ask_with_citations": false
  },
  "meeting_templates": false,
  "retention": {
    "delete": false,
    "auto_delete": false
  }
}
```

## 7. Core Runtime Types
### 7.1 `JobStatus`
```json
{
  "job_id": "job_123",
  "state": "queued|running|succeeded|failed|cancelled",
  "progress": 0.42,
  "message": "Transcribing chunk 3/8",
  "error": null
}
```

### 7.2 `InsertionResult`
Aligned with runtime insertion outcomes:
```json
{
  "outcome": "inserted|copied_to_clipboard|failed",
  "method": "paste|fallback_copy|clipboard_only",
  "target_app": {
    "bundle_id": "com.apple.mail",
    "name": "Mail"
  },
  "context_pack": {
    "status": "Off|Ready|Limited|Blocked",
    "reason": "nullable_reason_code",
    "source": {
      "adapter_id": "generic|ide|mail|browser",
      "app_class": "generic|ide|mail|browser",
      "bundle_id": "com.apple.mail"
    },
    "metrics": {
      "capture_duration_ms": 0,
      "payload_bytes": 128,
      "character_count": 128,
      "token_budget": 320,
      "token_estimate": 32
    }
  },
  "reason": null
}
```

## 8. Methods and Schemas
All methods below are v1 unless marked reserved.

## 8.1 `system.*`
### `system.ping`
- params: `{}`
- result: `{ "ok": true }`

### `system.health`
- params: `{}`
- result:
```json
{
  "ok": true,
  "recording_state": "idle|dictation|meeting",
  "active_jobs": 0,
  "uptime_seconds": 123
}
```

### `system.version`
- params: `{}`
- result: section 6.1

### `system.capabilities`
- params: `{}`
- result: section 6.2

Example:
```json
{"jsonrpc":"2.0","id":"1","method":"system.health","params":{}}
```

## 8.2 `dictation.*`
### `dictation.start`
- params:
```json
{
  "language": "en",
  "mode": "raw|polish"
}
```
- result:
```json
{
  "job": { "job_id": "job_dict_1", "state": "running", "progress": 0.0, "message": null, "error": null }
}
```

### `dictation.stop`
- params: `{}`
- result:
```json
{
  "ref": { "kind": "dictation", "id": "<uuid>" },
  "job": { "job_id": "job_dict_1", "state": "succeeded", "progress": 1.0, "message": null, "error": null }
}
```

### `dictation.cancel`
- params: `{}`
- result:
```json
{
  "job": { "job_id": "job_dict_1", "state": "cancelled", "progress": 0.0, "message": null, "error": null }
}
```

### `dictation.status`
- params: `{}`
- result:
```json
{
  "is_active": false,
  "job": null,
  "context_pack": null
}
```

## 8.3 `meetings.*`
### `meetings.start`
- params:
```json
{
  "capture": { "system_audio": true, "microphone": true },
  "title": "optional"
}
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "job": { "job_id": "job_meeting_1", "state": "running", "progress": 0.0, "message": null, "error": null }
}
```

### `meetings.stop`
- params:
```json
{ "meeting_id": "<uuid>" }
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "job": { "job_id": "job_meeting_1", "state": "running", "progress": 0.0, "message": "transcribing", "error": null }
}
```

### `meetings.cancel`
- params:
```json
{ "meeting_id": "<uuid>" }
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "job": { "job_id": "job_meeting_1", "state": "cancelled", "progress": 0.0, "message": null, "error": null }
}
```

### `meetings.status`
- params:
```json
{ "meeting_id": "<uuid>" }
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "status": "idle|recording|transcribing|completed|failed|cancelled",
  "live_segment_count": 0,
  "live_last_end_ms": 0,
  "is_finalizing": false,
  "job": null
}
```

### `meetings.list`
- params:
```json
{ "limit": 20, "cursor": null }
```
- result:
```json
{
  "items": [
    {
      "ref": { "kind": "meeting", "id": "<uuid>" },
      "title": "Weekly sync",
      "started_at": "2026-02-13T16:00:00Z",
      "duration_seconds": 1800,
      "status": "completed"
    }
  ],
  "next_cursor": null
}
```

### `meetings.get`
- params:
```json
{ "meeting_id": "<uuid>" }
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "title": "Weekly sync",
  "started_at": "2026-02-13T16:00:00Z",
  "duration_seconds": 1800,
  "status": "completed",
  "transcript": "...",
  "segments": [
    {
      "index": 0,
      "start_ms": 0,
      "end_ms": 1200,
      "text": "Hello.",
      "speaker": null,
      "source_type": "microphone|system|merged"
    }
  ]
}
```

### `meetings.search`
- params:
```json
{ "query": "API design", "limit": 10 }
```
- result:
```json
{
  "items": [
    {
      "ref": { "kind": "meeting", "id": "<uuid>" },
      "snippet": "...discuss API contract...",
      "score": null
    }
  ]
}
```

`score` is nullable and only meaningful when `knowledge.semantic_search=true`.

## 8.4 `transcripts.*`
`transcripts.*` is a cross-kind projection over dictation + meeting transcripts.

### `transcripts.list`
- params:
```json
{ "kinds": ["dictation", "meeting"], "limit": 20, "cursor": null }
```
- result:
```json
{
  "items": [
    { "ref": { "kind": "dictation", "id": "<uuid>" }, "title": "Dictation", "started_at": "...", "duration_seconds": 30, "status": "completed" }
  ],
  "next_cursor": null
}
```

### `transcripts.get`
- params:
```json
{ "ref": { "kind": "meeting", "id": "<uuid>" } }
```
- result:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "text_raw": "...",
  "text_polish": "...",
  "segments": []
}
```

### `transcripts.latest`
- params:
```json
{ "kind": "dictation|meeting|any" }
```
- result:
```json
{ "ref": { "kind": "dictation", "id": "<uuid>" } }
```

### `transcripts.search`
- params:
```json
{ "query": "api", "kinds": ["dictation", "meeting"], "limit": 10 }
```
- result:
```json
{
  "items": [
    {
      "ref": { "kind": "meeting", "id": "<uuid>" },
      "snippet": "...",
      "score": null
    }
  ]
}
```

### `transcripts.import`
Binds uploaded bytes to transcription/import pipeline.

- params:
```json
{
  "transfer_id": "xfer_in_1",
  "target_kind": "dictation|meeting",
  "filename": "call.m4a",
  "language": "en",
  "mode": "raw|polish"
}
```
- result:
```json
{
  "ref": { "kind": "dictation|meeting", "id": "<uuid>" },
  "is_partial": false,
  "job": { "job_id": "job_import_1", "state": "succeeded|running", "progress": 1.0, "message": null, "error": null }
}
```

For `target_kind=meeting`, the response returns after the meeting session is visible and import handoff is running:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "is_partial": false,
  "job": { "job_id": "job_import_1", "state": "running", "progress": 0.0, "message": "transcribing", "error": null }
}
```

The `filename` param is sanitized and preserved for the temporary upload extension, imported meeting title default, and meeting `metadata.json` `originalFilename`. Content type is only a fallback when `filename` is missing or unsupported.
Upload transfers are spooled to Dettivo's import directory while chunks arrive, so large meeting recordings do not need to remain in IPC memory before the meeting import handoff.

Known conflict errors for imports:
- `CONFLICT` with detail kinds:
  - `importAlreadyInProgress`
  - `importBlockedByActiveSession`

### `transcripts.export`
- params:
```json
{
  "ref": { "kind": "meeting", "id": "<uuid>" },
  "format": "txt|md|json|srt|vtt",
  "transfer_id": "xfer_out_1"
}
```
- result:
```json
{
  "transfer_id": "xfer_out_1",
  "content_type": "text/markdown",
  "filename": "meeting.md"
}
```

Format validity must be checked against capability scope (`dictation_export` vs `meeting_export`).

## 8.5 `insert.*`
### `insert.perform`
- params:
```json
{
  "mode": "raw|polish|clipboard_only",
  "text": "optional if source_ref provided",
  "source_ref": { "kind": "dictation|meeting", "id": "<uuid>" },
  "expected_target_bundle_id": "optional",
  "expected_target_pid": "optional"
}
```
- result: `InsertionResult`

If both `text` and `source_ref` are provided, `text` wins.
If guard fields mismatch frontmost target, return `CONFLICT`.

## 8.6 `polish.*`
### `polish.rules.list`
- params: `{}`
- result:
```json
{ "rules": [] }
```

### `polish.rules.create`
- params:
```json
{ "name": "Professional Tone", "enabled": true, "content": "Use concise professional phrasing" }
```
- result:
```json
{ "rule_id": "<uuid>" }
```

### `polish.rules.update`
- params:
```json
{ "rule_id": "<uuid>", "enabled": true, "content": "updated" }
```
- result:
```json
{ "rule_id": "<uuid>", "updated": true }
```

### `polish.rules.delete`
- params:
```json
{ "rule_id": "<uuid>" }
```
- result:
```json
{ "deleted": true }
```

### `polish.presets.list`
- params: `{}`
- result:
```json
{ "presets": [] }
```

### `polish.test`
- params:
```json
{
  "input": "um update readme",
  "preset": "code",
  "rules": ["Professional Tone"]
}
```
- result:
```json
{
  "output": "Please update the README.",
  "applied_rules": ["Professional Tone"],
  "model": "mlx-community/Qwen3-4B-4bit"
}
```

### `polish.apps.list`
- params: `{}`
- result:
```json
{ "mappings": [] }
```

### `polish.apps.set`
- params:
```json
{ "bundle_id": "com.apple.mail", "preset": "email" }
```
- result:
```json
{ "bundle_id": "com.apple.mail", "preset": "email" }
```

## 8.7 `automation.*`
### `automation.jobs.create`
- params:
```json
{
  "trigger": "analysis_ready|weekly_digest",
  "meeting_id": "<uuid-when-analysis-ready>",
  "force": true
}
```
- result:
```json
{
  "job": {
    "id": "<uuid>",
    "idempotency_key": "recap:...|digest:...",
    "run_type": "recap|weekly_digest",
    "trigger": "analysis_ready|weekly_digest",
    "scope": "personal|organization",
    "meeting_id": "<uuid|null>",
    "actor_id": "<normalized-user>",
    "state": "queued|running|succeeded|failed",
    "attempts": 1,
    "retryable": false,
    "error_code": null,
    "error_message": null,
    "delivery_mode": "draft_only|auto_send|provider_send",
    "delivery_provider": "gmail|microsoft_graph|smtp|null",
    "provider_message_id": "<string|null>",
    "draft_subject": "<string|null>",
    "draft_body": "<string|null>",
    "created_at": "2026-02-15T11:00:00Z",
    "updated_at": "2026-02-15T11:00:00Z",
    "completed_at": "2026-02-15T11:00:00Z|null"
  }
}
```

### `automation.jobs.list`
- params:
```json
{ "limit": 20 }
```
- result:
```json
{ "items": [] }
```

### `automation.providers.list`
- params:
```json
{}
```
- result:
```json
{
  "items": [
    {
      "id": "<uuid>",
      "provider_kind": "gmail|microsoft_graph|smtp",
      "provider_name": "Gmail|Microsoft Graph|SMTP",
      "owner_id": "<normalized-user>",
      "sender_email": "sender@example.com",
      "sender_name": "Optional Name|null",
      "smtp_host": "smtp.example.com|null",
      "smtp_port": 465,
      "smtp_username": "smtp-user|null",
      "smtp_use_tls": true,
      "state": "connected|auth_failed|config_error|missing_credentials",
      "has_credentials": true,
      "last_error_code": "PROVIDER_AUTH_FAILED|null",
      "last_error_message": "Token expired|null",
      "last_validated_at": "2026-02-15T11:00:00Z|null",
      "created_at": "2026-02-15T11:00:00Z",
      "updated_at": "2026-02-15T11:00:00Z"
    }
  ]
}
```

### `automation.providers.connect`
- params:
```json
{
  "provider": "gmail|microsoft_graph|smtp",
  "sender_email": "sender@example.com",
  "sender_name": "Optional Name",
  "access_token": "<required for gmail/graph>",
  "refresh_token": "<optional for gmail/graph>",
  "smtp_host": "smtp.example.com",
  "smtp_port": 465,
  "smtp_username": "smtp-user",
  "smtp_password": "app-password",
  "smtp_use_tls": true
}
```
- result:
```json
{
  "provider": {
    "id": "<uuid>",
    "provider_kind": "gmail|microsoft_graph|smtp",
    "state": "connected"
  }
}
```

Provider send/runtime error codes used by `automation.jobs.create`:
- `PROVIDER_CONFIG_MISSING_ACCOUNT`
- `PROVIDER_CONFIG_MISSING_RECIPIENTS`
- `PROVIDER_CONFIG_MISSING_SECRET`
- `PROVIDER_AUTH_FAILED`
- `PROVIDER_RATE_LIMITED`
- `PROVIDER_TIMEOUT`
- `PROVIDER_UNAVAILABLE`
- `PROVIDER_REJECTED`
- `PROVIDER_INTERNAL_ERROR`

### `automation.providers.disconnect`
- params:
```json
{ "provider_id": "<uuid>" }
```
- result:
```json
{ "disconnected": true }
```

### `automation.macros.list`
- params:
```json
{}
```
- result:
```json
{
  "config": {
    "enabled": true,
    "macros": [
      {
        "id": "<string>",
        "trigger": "insert greeting",
        "enabled": true,
        "safety_policy": "always_confirm|first_use_confirm|auto_allow",
        "action": {
          "kind": "insert_text|app_command|shell_command",
          "target": "<text|command_id|executable>",
          "arguments": [],
          "preview": "Insert text: hello team"
        }
      }
    ],
    "allowlist": {
      "app_commands": ["open_settings"],
      "shell_commands": ["open"]
    },
    "remembered_macro_ids": []
  }
}
```

### `automation.macros.upsert`
- params:
```json
{
  "macro_id": "<optional-id>",
  "trigger": "insert greeting",
  "action_kind": "insert_text|app_command|shell_command",
  "target": "<text|command_id|executable>",
  "arguments": [],
  "safety_policy": "always_confirm|first_use_confirm|auto_allow",
  "enabled": true,
  "config_enabled": true,
  "allowlist_app_commands": ["open_settings"],
  "allowlist_shell_commands": ["open"]
}
```
- result:
```json
{ "config": { "...": "same as automation.macros.list" } }
```

### `automation.macros.delete`
- params:
```json
{ "macro_id": "<string>" }
```
- result:
```json
{ "deleted": true, "config": { "...": "same as automation.macros.list" } }
```

### `automation.macros.preview`
- params:
```json
{ "transcript": "please insert greeting now" }
```
- result:
```json
{
  "preview": {
    "macro": { "...": "macro payload" },
    "status": "preview|blocked",
    "dry_run": true,
    "applied": false,
    "action_preview": "Insert text: hello team",
    "transformed_text": "please hello team now",
    "message": null
  }
}
```

### `automation.macros.run`
- params:
```json
{
  "trigger": "insert greeting",
  "transcript": "please insert greeting now",
  "dry_run": false
}
```
- result:
```json
{
  "result": {
    "macro": { "...": "macro payload" },
    "status": "run|blocked|failed|cancelled|dropped|preview",
    "dry_run": false,
    "applied": true,
    "action_preview": "Insert text: hello team",
    "transformed_text": "please hello team now",
    "message": null
  }
}
```

### `automation.macros.audit.list`
- params:
```json
{ "limit": 20 }
```
- result:
```json
{
  "items": [
    {
      "id": "<uuid>",
      "macro_id": "<string|null>",
      "trigger": "insert greeting",
      "action_kind": "insert_text|app_command|shell_command",
      "action_target": "hello team",
      "event_kind": "trigger|run|blocked|failed|cancelled|dropped",
      "message": null,
      "created_at": "2026-02-17T12:00:00Z"
    }
  ]
}
```

## 8.8 `events.*`
### `events.subscribe`
- params:
```json
{ "topics": ["dictation.state", "meeting.state", "job.progress"], "buffer": 256 }
```
- result:
```json
{ "subscription_id": "sub_1", "buffer": 256 }
```

### `events.unsubscribe`
- params:
```json
{ "subscription_id": "sub_1" }
```
- result:
```json
{ "unsubscribed": true }
```

Server notification shape:
```json
{
  "jsonrpc": "2.0",
  "method": "events.notify",
  "params": {
    "subscription_id": "sub_1",
    "topic": "job.progress",
    "timestamp": "2026-02-13T17:00:00Z",
    "payload": { "job_id": "job_dict_1", "progress": 0.6 }
  }
}
```

`topic = "meeting.state"` payload includes bounded live metadata:
```json
{
  "kind": "meeting",
  "state": "recording|transcribing|completed|failed|cancelled",
  "meeting_id": "<uuid>",
  "live_segment_count": 24,
  "live_last_end_ms": 91234,
  "is_finalizing": false
}
```

Overflow behavior:
- on subscriber buffer overflow, oldest events may be dropped
- server emits `topic=events.overflow` with drop count

## 8.9 `transfer.*`
Used for large payload upload/download.

### `transfer.begin`
- params:
```json
{ "direction": "upload|download", "content_type": "audio/m4a", "size_hint": 4200000 }
```
- result:
```json
{ "transfer_id": "xfer_1", "chunk_max_bytes": 1048576, "expires_at": "2026-02-13T17:10:00Z" }
```

### `transfer.chunk`
Upload chunk (client -> server).
- params:
```json
{ "transfer_id": "xfer_1", "seq": 1, "data_b64": "..." }
```
- result:
```json
{ "accepted": true, "next_seq": 2 }
```

### `transfer.pull`
Download chunk (client <- server).
- params:
```json
{ "transfer_id": "xfer_out_1", "seq": 1 }
```
- result:
```json
{ "seq": 1, "data_b64": "...", "eof": false }
```

### `transfer.commit`
- params:
```json
{ "transfer_id": "xfer_1", "total_chunks": 5, "sha256": "<hex>" }
```
- result:
```json
{ "committed": true }
```

Commit semantics:
- upload: client calls after last `transfer.chunk`
- download: server-side transfer is ready at begin/export binding; client MAY call commit as final ack (`committed=true`)

### `transfer.cancel`
- params:
```json
{ "transfer_id": "xfer_1", "reason": "client_abort" }
```
- result:
```json
{ "cancelled": true }
```

## 9. Reserved Namespaces and Methods
These may return `NOT_IMPLEMENTED` until enabled by capability flags.

Reserved namespaces:
- `knowledge.*` (`fn-29`, `fn-10`)
- `meeting.templates.*` (`fn-30`)
- `retention.*` (`fn-11`)

Reserved methods (shape protected now):
- `knowledge.search`
- `knowledge.ask`
- `meeting.templates.list`
- `meeting.templates.set_default`
- `meetings.delete`
- `retention.policy.get`
- `retention.policy.set`

`meetings.delete` reserved params:
```json
{
  "meeting_id": "<uuid>",
  "artifact_policy": "transcript_only|transcript_and_audio|all"
}
```

## 10. Adapter Requirements
## 10.1 CLI
- must map 1:1 to IPC methods and semantics
- must support human output and `--json` machine mode
- exit codes (normative):
  - `0` success
  - `1` generic/runtime failure
  - `2` app/IPC unavailable
  - `3` permission denied
  - `4` invalid command/args
  - `5` timeout/interrupted operation

## 10.2 MCP
- tools/resources map to IPC contract
- default operation should work in IPC peer mode (no token plumbing)
- in hardened mode, token configuration guidance is required

### 10.2.1 Tool naming and schema policy
- MCP tool input schemas use IPC param keys (snake_case) from section 8.
- Representative tool set includes:
  - read: `get_status`, `list_transcripts`, `get_transcript`, `search_transcripts`, `get_latest_transcript`
  - control/write: `start_dictation`, `stop_session`, `cancel_session`, `start_meeting`, `import_audio`, `export_transcript`, `insert_transcript`
  - polish config writes: `set_polish_app`
- Tool outputs are bounded for agent safety (list/field truncation) while preserving JSON structure.

### 10.2.2 Resource retrieval patterns
- list pattern: `resources/list` returns `status://current` + recent transcript/meeting URIs.
- get pattern: `resources/read` resolves `transcript://{id}` and `meeting://{id}`.
- search/latest patterns: use resource templates and `resources/read` for:
  - `transcripts://search/{query}?kinds=...&limit=...`
  - `transcripts://latest/{kind}`

### 10.2.3 Unavailable/hardened guidance
- On IPC unavailable, MCP tool calls return actionable error text:
  - start app
  - verify/override socket path (`DETTIVO_IPC_SOCKET`, `--socket`)
- On hardened auth mismatch, MCP tool calls guide token setup:
  - `DETTIVO_IPC_TOKEN`
  - `--token` / `--token-file`

### 10.2.4 Transport compatibility (fn-2.6)
- MCP stdio primary framing is line-delimited JSON (one JSON-RPC message per line, `\n` terminator).
- Compatibility: server auto-detects client framing and mirrors responses:
  - line-delimited JSON in → line-delimited JSON out
  - `Content-Length` framed message in → `Content-Length` framed message out
- This keeps Cursor/Claude Desktop clients (line-delimited) and legacy framed clients interoperable during `initialize`.

## 10.3 REST shim
- disabled by default
- loopback only
- token required
- same semantics and error `app_code` values as IPC
- app settings UX may provision keychain token automatically when enabling REST shim

## 11. Conformance Review Checklist
- [ ] every v1 method has explicit params and result schema
- [ ] at least one request/response example exists per method group
- [ ] error model includes `app_code`, retryability, and optional runtime mapping details
- [ ] capability flags describe auth mode, format scopes, and future feature availability
- [ ] transfer protocol supports both upload (`chunk`) and download (`pull`)
- [ ] import operation binds uploaded bytes to transcription pipeline
- [ ] insertion result shape aligns with runtime outcome model
- [ ] reserved methods return `NOT_IMPLEMENTED` until corresponding capability is enabled

## 12. Open Decisions
- whether hardened IPC mode should be Settings-driven or config/env-only
- adapter auto-launch behavior for app process
- final CLI command naming for transcript vs history projection
