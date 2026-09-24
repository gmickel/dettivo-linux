# The CLI, the MCP server and the REST shim

## Verdict

These are sensible adapters around a daemon-owned contract, but authentication, framing, failure reporting and transfer handling contain release-blocking defects. The most valuable structural change is to delete the duplicated socket and transfer implementations and give all three clients one tested transport layer. This read-only review covered HEAD `28b4b7d8c16a`, reproduced three MCP defects in the existing release binaries, and verified the CLI tree against the agent guide; `just build test lint` and socket integration tests were not run because the sandbox prohibits build writes and socket access.

## Findings

### F1. A failed policy lookup turns meeting deletion into “delete everything”

- **Kind:** bug
- **Where:** `crates/dettivo-cli/src/meetings.rs:362` — `.ok()` followed by `.unwrap_or_else(|| "all".to_string())`.
- **Why:** A timeout, authorization error or malformed configuration response becomes the most destructive artifact policy. If the subsequent delete succeeds, someone who configured `transcript_only` can lose the meeting’s audio, notes and row.
- **Change:** Propagate the lookup failure immediately. Move resolution of an omitted policy into the daemon’s delete operation, removing this client-side policy decision and its extra request.
- **Risk:** Preserve explicit policy arguments and record any additive contract change. Test a failed configuration lookup followed by an otherwise available daemon, asserting that no delete occurs.

### F2. REST resolves a token for HTTP authentication but can omit it from IPC

- **Kind:** bug
- **Where:** `crates/dettivo-cli/src/rest.rs:106` — `token: client.token.clone()`; `crates/dettivo-cli/src/client.rs:48` — `std::env::var("DETTIVO_IPC_TOKEN")`.
- **Why:** REST also resolves tokens through Secret Service and the configured token file. The socket client only receives flags or the environment token. Consequently, process-hosted REST can accept the correct HTTP token and then receive `UNAUTHORIZED_CLIENT` from a hardened daemon.
- **Change:** Pass the resolved shared token into `SocketBackend`, and use that authenticated client when loading REST settings.
- **Risk:** Test both hosting modes with `peer_token`, covering environment, Secret Service and token-file resolution independently.

### F3. Process-hosted REST uploads exceed the daemon’s default line limit

- **Kind:** bug
- **Where:** `crates/dettivo-rest/src/stream.rs:106` — `begin["chunk_max_bytes"]`; `crates/dettivod/src/platform.rs:17` — `chunk_max_bytes: 1_048_576`; `crates/dettivo-core/src/config/schema.rs:262` — `max_line_bytes: 1_048_576`.
- **Why:** REST sends the full advertised raw chunk size after base64 encoding. A 1 MiB chunk becomes 1,398,104 bytes before the JSON envelope, exceeding the socket’s 1 MiB request limit. Daemon-hosted REST bypasses socket framing, so the same upload behaves differently between hosts.
- **Change:** Calculate upload chunks against both the raw transfer limit and the encoded IPC envelope limit. Use the same calculation across clients.
- **Risk:** Test uploads crossing chunk boundaries through both hosting modes, including a reduced IPC line limit. These tests can exercise transfers without an inference model.

### F4. Invalid MCP arguments can select the wrong session to cancel

- **Kind:** contract
- **Where:** `crates/dettivo-mcp/src/protocol.rs:208` — arguments are filtered with `.filter(|a| a.is_object())` and replaced with `{}`; `crates/dettivo-mcp/src/tools/dispatch.rs:199` — `client.call(&format!("dictation.{action}"), json!({}))`.
- **Why:** Non-object arguments become an empty argument object. A supplied numeric `meeting_id` also disappears through `str_arg`, routing `cancel_session` to dictation instead of rejecting the malformed meeting request. The published schemas do not validate execution.
- **Change:** Reject supplied arguments of the wrong type before applying defaults or selecting a daemon method. Keep “absent” distinct from “present but invalid.”
- **Risk:** Preserve documented defaults and intentional coercions such as insertion PID conversion. Test malformed cancellation requests with a recording dictation and assert that no daemon mutation occurs.

### F5. Malformed Unicode can panic protocol handling

- **Kind:** bug
- **Where:** `crates/dettivo-mcp/src/resources.rs:178` — `&text[i + 1..i + 3]`; `crates/dettivo-rest/src/auth.rs:23` — `auth[..7].eq_ignore_ascii_case("bearer ")`.
- **Why:** Both slice UTF-8 strings at unchecked byte offsets. `resources/read` with `transcripts://search/%aé` reproducibly terminates MCP with exit 101. An authorization value such as `aaaaaaé` reaches the same defect in REST; its handler panic becomes a 500 instead of an authentication refusal.
- **Change:** Decode percent escapes from bytes and inspect the authorization prefix using safe byte access. Return ordinary protocol errors for malformed input.
- **Risk:** Test incomplete escapes, multibyte characters beside escapes, and non-ASCII authorization prefixes. Verify that subsequent requests still work.

### F6. MCP does not enforce its outgoing message cap

- **Kind:** contract
- **Where:** `crates/dettivo-mcp/src/protocol.rs:122` — `transport::write_message(&mut output, reader.framing(), &response)`; `crates/dettivo-mcp/src/bounds.rs:21` — “Bytes accepted per transport message in either direction.”
- **Why:** The cap configures the reader, but responses are serialized and written without a byte check. Per-field truncation does not bound the complete envelope, especially when results appear twice. A 1,999,948-byte initialize request reproducibly produced a 2,000,077-byte JSON response, exceeding the default cap.
- **Change:** Enforce the cap after serializing every complete response. Return a bounded error retaining the request ID when the result cannot fit.
- **Risk:** Test both framings, small configured caps, multibyte text, large tool results and resource listings. Existing incoming-size tests do not cover this.

### F7. REST retains every completed connection task until shutdown

- **Kind:** bug
- **Where:** `crates/dettivo-rest/src/server.rs:135` — `tasks.spawn(serve(...))`; `crates/dettivo-rest/src/server.rs:147` — `while tasks.join_next().await.is_some() {}` occurs only after the accept loop ends.
- **Why:** Completed tasks remain in the `JoinSet` until joined. A resident daemon accumulates task records as connections come and go, including unauthenticated connections.
- **Change:** Reap completed tasks inside the accept loop. Bound active connections so an unauthenticated peer cannot create unlimited concurrent request buffers.
- **Risk:** Exercise many short connections and verify that retained task count returns to zero while the listener remains running.

### F8. REST’s request timeout leaves work running and does not bound the whole request

- **Kind:** bug
- **Where:** `crates/dettivo-rest/src/server.rs:157` — timeout around `read_request`; `crates/dettivo-rest/src/server.rs:185` — a fresh timeout around `spawn_blocking`; `crates/dettivo-rest/src/server.rs:198` — response writing has no timeout.
- **Why:** Reading and handling each receive the full budget, and writing can block indefinitely. Timing out the blocking task drops its handle without stopping it, so transfer steps or mutations can continue after HTTP reports failure. A retry can overlap the original operation.
- **Change:** Carry one deadline through reading, dispatch and writing. Stop scheduling further transfer steps after expiry, and distinguish an unknown mutation outcome from a confirmed failure. Keep blocking work tracked through completion.
- **Risk:** Use a backend that pauses before a mutation, a slow upload and a client that stops reading. Verify deadlines, shutdown behavior and whether late mutations occur.

### F9. A refused MCP framing switch still executes its payload

- **Kind:** contract
- **Where:** `crates/dettivo-mcp/src/transport.rs:171` — only the current line is drained; `crates/dettivo-mcp/src/transport.rs:328` — the switch test checks one error and then stops reading.
- **Why:** In line mode, a `Content-Length` header produces an invalid-request response, but its following JSON payload is accepted as another line. The release binary returned an error and then successfully answered the switched `ping`. A tool payload would likewise reach dispatch.
- **Change:** Consume the entire rejected frame before resuming the original framing. If safe recovery is impossible, terminate the session and record that contract change.
- **Risk:** Test switches in both directions with fragmented input. Assert that the rejected payload never executes and that recovery preserves only subsequent valid messages.

### F10. Compositor dictation overrides configured mode and implements toggle outside the daemon

- **Kind:** simplify
- **Where:** `crates/dettivo-cli/src/commands.rs:269` — absent mode becomes `"raw"`; `crates/dettivo-cli/src/commands.rs:279` — `dictation.status` precedes a separate start or stop; `crates/dettivod/src/dictation.rs:181` — omitted mode otherwise resolves from `d.mode`.
- **Why:** Generated compositor bindings call the CLI without a mode, so they force raw mode while daemon-side hotkeys can use the configured mode. Toggle also makes a decision from a snapshot that can change before its next request.
- **Change:** Give compositor actions a daemon-owned operation that resolves configured defaults and serializes the toggle decision. Delete the CLI’s status-then-action branch. Preserve explicitly documented MCP defaults separately.
- **Risk:** Test non-raw configuration through compositor, portal and evdev paths, plus concurrent toggles and explicit mode overrides.

### F11. MCP host configuration can discard existing servers

- **Kind:** bug
- **Where:** `crates/dettivo-mcp/src/hosts.rs:199` — `if !doc.contains_table("mcp_servers")` replaces the entry; `crates/dettivo-mcp/src/hosts.rs:245` — `read_to_string(path).unwrap_or_default()`.
- **Why:** A valid TOML inline table is not a regular `Table`, so an inline `mcp_servers` value is replaced and its other servers disappear. Separately, every file-read error is treated as an empty configuration, allowing a subsequent write to overwrite content that was never successfully read.
- **Change:** Preserve or explicitly reject existing inline-table representations. Treat only `NotFound` as an empty file, propagate other read failures, and replace files atomically after a successful merge.
- **Risk:** Test regular and inline TOML tables, unrelated servers, unreadable files and interrupted writes. Failure must leave the original file intact.

### F12. Three transfer implementations buffer whole files and disagree about cleanup

- **Kind:** simplify
- **Where:** `crates/dettivo-cli/src/transfer.rs:98` — `let mut bytes = Vec::new()`; `crates/dettivo-mcp/src/tools/transfer.rs:151` — the same accumulation; `crates/dettivo-rest/src/stream.rs:182` — `let mut body = Vec::new()`; `crates/dettivo-mcp/src/tools/transfer.rs:77` — upload errors return directly through `?`.
- **Why:** Chunked transfers still materialize entire downloads in each client. CLI and MCP also read whole uploads before transferring them. Cleanup differs: MCP export cancels on failure, MCP import does not, and CLI transfers return from several failure paths without cancellation.
- **Change:** Replace the three loops with shared transfer primitives over readers and writers, incremental hashing, and one explicit cancellation path. Stream file output through a temporary destination; retain only the bounded preview MCP needs.
- **Risk:** Check large-file memory use, interrupted uploads, disk-full writes, failed imports and failed final acknowledgments. Preserve existing output files until download completion.

### F13. Waiting for an import depends on its position in the newest 100 items

- **Kind:** bug
- **Where:** `crates/dettivo-cli/src/history.rs:351` — `"limit": 100, "cursor": null`; `crates/dettivo-cli/src/transfer.rs:320` — the same query; `crates/dettivod/src/handlers/transcripts.rs:131` — item facts already expose `status`.
- **Why:** Both wait loops search one listing page and report “is no longer listed” when the target is absent. A long-running item can leave that page as newer items arrive, although it still exists and its job continues.
- **Change:** Poll `transcripts.get` by reference and read its item status. Delete the duplicated listing searches.
- **Risk:** Test a running item behind more than 100 newer rows, cancellation, deletion and normal completion.

### F14. Doctor can report success when required probes failed

- **Kind:** simplify
- **Where:** `crates/dettivo-cli/src/doctor.rs:139` — configuration errors become an “unreachable” note without clearing `healthy`; `crates/dettivo-cli/src/doctor.rs:152` — engine errors become `Null`; `crates/dettivo-cli/src/doctor.rs:168` — LLM engine errors also become `Null`.
- **Why:** After the initial health probe succeeds, later failures can leave the final exit code at zero. Error classes are also lost or mislabeled as daemon unavailability. Doctor’s 927 lines across collection and rendering contain repeated model summaries, engine presentation and client facts.
- **Change:** Track success, failure and unknown explicitly for each required probe. Move daemon-owned diagnostic aggregation behind the daemon contract, retain local service/socket checks in the CLI, and reduce the human report to failures plus concise readiness rows.
- **Risk:** Preserve the documented JSON surface during migration. Test failures after a successful health response, including authorization errors and engine-query timeouts.

### F15. Delete two copies of the Unix-socket client

- **Kind:** delete
- **Where:** `crates/dettivo-cli/src/client.rs:61`, `crates/dettivo-mcp/src/client.rs:117` and `crates/dettivo-rest/src/backend.rs:66` each implement `fn call`, connection setup, authentication, serialization and response parsing.
- **Why:** The same transport is maintained three times, with different error representations and timeout mappings. All three read an unbounded response line. CLI subscription setup adds another variant that sets only a write timeout before waiting for its initial response.
- **Change:** Keep one shared transport with typed transport failures, explicit response bounds and request deadlines. Leave CLI exit codes, MCP guidance and REST status mapping in their adapters.
- **Risk:** Preserve existing error text and authentication precedence where contractual. Test EOF, malformed responses, delayed subscription acknowledgment, oversized responses and slowly delivered responses.

### F16. `--json` produces no JSON for locally detected failures

- **Kind:** contract
- **Where:** `crates/dettivo-cli/src/output.rs:183` — JSON is printed only under `if let Some(rpc) = &failure.rpc`; `docs/guides/agents.md:177` — “With `--json` a failure prints the JSON-RPC error object on standard output”.
- **Why:** Argument validation, socket failures, timeouts and local file errors generally have no daemon error object. For example, `dettivo --json call system.ping '[]'` reproducibly exits 4 with empty stdout, preventing a JSON consumer from reading the promised error shape.
- **Change:** Give adapter-originated failures a documented structured error representation while preserving daemon errors unchanged. Apply it consistently to command parsing as well.
- **Risk:** Test stdout, stderr and exit code together for each failure class. Preserve quiet-mode precedence.

### F17. `mcp check` reports framing support without exercising MCP

- **Kind:** test
- **Where:** `crates/dettivo-cli/src/mcp.rs:162` — calls `system.version`; `crates/dettivo-cli/src/mcp.rs:179` — reports framing names from enum constants.
- **Why:** The command queries the daemon and counts locally constructed definitions. It never launches or initializes an MCP server and never sends a framed message, despite being described as an initialization/framing check.
- **Change:** Run a bounded subprocess handshake through `mcp serve`, followed by `tools/list` and a harmless daemon-backed call. Report which transport checks actually ran.
- **Risk:** Test server startup failure, stdout contamination, malformed framing and handshake timeout. Ensure the check always cleans up its child.

### F18. REST status labels every non-200 response as an authentication failure and exits successfully

- **Kind:** bug
- **Where:** `crates/dettivo-cli/src/rest.rs:137` — authorization is reduced to `reply.status == 200`; `crates/dettivo-cli/src/rest.rs:182` — any listening response returns `Ok(())`; `crates/dettivo-cli/src/rest.rs:206` — all other responses print “token is refused (401)”.
- **Why:** A process-hosted shim whose daemon is unavailable returns 503. Status reports that as a refused token and exits zero. Actual 401 responses also exit zero, undermining its use as a readiness check.
- **Change:** Retain the actual HTTP status and error body in the report. Distinguish authentication refusal, unavailable backend and other failures, and map them to the appropriate CLI failure code.
- **Risk:** Test 200, 401, 500, 503 and connection refusal against both hosting modes.

## Keep

- `crates/dettivo-rest/src/routes.rs` — catalog-based routing and daemon-side parameter validation keep business logic out of HTTP.
- `crates/dettivo-rest/src/status.rs` — one explicit status map preserves the daemon’s error object.
- `crates/dettivo-rest/src/server.rs` — loopback-only binding and refusal to start without a token are sound boundaries.
- `crates/dettivo-mcp/src/bounds.rs` — character-aware truncation and explicit partial-result markers are useful; retain them alongside a complete-message cap.
- `crates/dettivo-cli/src/docs.rs` and `docs/guides/agents.md` — generating the CLI tree from Clap works; the release binary’s tree matches the guide.
- `crates/dettivo-cli/src/main.rs` — the generic `call` escape hatch avoids needing a convenience command for every daemon method.

## Questions for the owner

- Do any supported hosts still require `Content-Length` MCP framing? If it is only historical compatibility, deleting it would remove a parser and its recovery rules.
- Should doctor success mean that speech actually works, or only that the daemon answers, configuration validates and no engine reports degradation? That decision should precede stronger readiness checks.
- Is the agent guide now the canonical CLI reference? The separately requested CLI topic page is absent.