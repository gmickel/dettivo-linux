# Contract fixtures

These files state what it means for a Dettivo server to conform to the IPC v1 contract, in plain JSON any harness can replay. The Rust unit job proves every file round-trips byte-stable through `dettivo-proto`; the QA rig sends the same requests to a live `dettivod` socket and compares the answers.

## Layout

One directory per namespace, one file per method and per error case:

```
fixtures/
  system/
    health.json                     # system.health, success
    ping.error-unauthorized.json    # system.ping, an error case
  polish/
    rules.list.json                 # polish.rules.list
  knowledge/
    search.error-not-implemented.json   # a reserved method
```

The file name is the method with its namespace removed, plus `.error-<case>` for an error case. The `method` field inside must agree with the path; a file that breaks either rule fails the suite.

## File shape

```json
{
  "method": "transcripts.export",
  "capabilities": { "formats.meeting_export": ["txt", "md", "json", "srt", "vtt"] },
  "request":  { "jsonrpc": "2.0", "id": "1", "method": "transcripts.export", "params": { "...": "..." } },
  "response": { "jsonrpc": "2.0", "id": "1", "result": { "...": "..." } }
}
```

- `capabilities` is required on every file. It maps dotted paths into the `system.capabilities` result to the value the fixture needs there; an empty object means the fixture holds on any server. Conformance is always conditional on declared capabilities: a replay skips and reports a fixture whose flags the server does not declare, and never counts it as passed.
- Exactly one of `response` or `error` is present. A reserved method always carries `error` with `NOT_IMPLEMENTED` and code `-32014`.
- Ids are fixed strings so a replay can match answers to requests; a live server echoes whatever id it receives.
- `config/print_default.json` carries the whole default `config.toml`, so the default file is part of the contract a daemon is checked against.

## Replaying against a live socket

For each fixture: read the server's `system.capabilities`, skip the fixture when a required flag is absent or differs, send `request` as one line, read one line, and compare it with `response` or `error` using tolerant matching for the values a server generates on the spot (`job_id`, `policy_hash`, `transfer_id`, `subscription_id`, timestamps, `expires_at`, `build`, `app_version`, `uptime_seconds`, `path`, `latency_ms`; the `platform` block, the `rest` block's `enabled` and `port`, and every `config.path` value describe the machine). Everything else, including the numeric error code and `app_code`, must match exactly.
