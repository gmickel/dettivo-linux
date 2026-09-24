# Cleanup: security boundaries (6 review findings)

## Conversation Evidence

> user (2026-09-06): "might as well do our astra reviewing ... send out multiple subagents that can invoke codex ... to review parts of the app in parallel" then "yes, when everything is back do that, fix all, parallize as necessary. also if astra says something was overengineered too"
> The findings below come from ten gpt-6-astra reviews of main 28b4b7d (one per area, read-only, the reports under `_factory/review/reports/` in the worktree root and `~/.cache/dettivo/astra-review/`), grouped by theme across areas.

## Goal & Context

<!-- Goal & Context: 40% [user], 60% [paraphrase] -->

Tokens, file modes, trust boundaries and protocol parsing hold at every edge. Each finding is a claim by a reviewer that never ran the gate: the worker verifies it against the code and a test first, fixes what holds with a regression test, and rejects what does not with written evidence in the task summary, so the record says which it was. Over-engineering the reviewer named is removed, not defended.

## Architecture & Data Models

<!-- Architecture & Data Models: 100% [paraphrase] -->

- The findings, by area, with the reviewer's evidence. Every path is on main 28b4b7d. A decision that changes is recorded in ADR 0050 (one record for this theme, naming the findings it closes and the ADRs it amends). [paraphrase]

### daemon

- **daemon/F8** (bug): Transfer files rely on the caller’s umask for confidentiality
  - Where: `crates/dettivod/src/transfers.rs:156`: `std::fs::create_dir_all(dir)`; line 165: `std::fs::write(&path, b"")`; line 340: `std::fs::write(&t.path, bytes)`.
  - Why: Upload staging and rendered exports contain audio or transcript content, but creation specifies no private permissions. Under umask `022`, newly created directories and files can be `0755` and `0644`. Where parent directories are traversable, this bypasses the privacy provided by the socket’s permissions.
  - Change: Create dedicated transfer directories as `0700` and files as `0600`, using safe file creation. Enforce this in the daemon so standalone execution receives the same protection.
  - Risk: Existing files and directories need explicit treatment. Check resulting permissions under permissive umasks and verify upload, export and expiry still work.

- **daemon/F19** (bug): Valid request paths violate the stated log-content boundary
  - Where: `crates/dettivod/src/transfers.rs:289`: `tracing::debug!(transfer = %id, reason, "transfer cancelled")`; `crates/dettivod/src/handlers/llm.rs:90`: `tracing::info!(endpoint = %canonical, "llm: remote endpoint trusted")`.
  - Why: `reason` is arbitrary client-provided text, and the endpoint is a configuration value. Both contradict the logging module’s explicit prohibition on request parameters and configuration values. The marker test’s claimed coverage of every request path does not include these valid operations.
  - Change: Remove the cancellation reason and complete endpoint from logs; retain operation names, counts and controlled status codes. Extend marker testing with successful transfer cancellation and endpoint trust requests.
  - Risk: Preserve useful diagnostics without copying user content. Run valid requests containing unique markers at trace level and assert that none reaches captured logs.

### agent-surfaces

- **agent-surfaces/F2** (bug): REST resolves a token for HTTP authentication but can omit it from IPC
  - Where: `crates/dettivo-cli/src/rest.rs:106` — `token: client.token.clone()`; `crates/dettivo-cli/src/client.rs:48` — `std::env::var("DETTIVO_IPC_TOKEN")`.
  - Why: REST also resolves tokens through Secret Service and the configured token file. The socket client only receives flags or the environment token. Consequently, process-hosted REST can accept the correct HTTP token and then receive `UNAUTHORIZED_CLIENT` from a hardened daemon.
  - Change: Pass the resolved shared token into `SocketBackend`, and use that authenticated client when loading REST settings.
  - Risk: Test both hosting modes with `peer_token`, covering environment, Secret Service and token-file resolution independently.

- **agent-surfaces/F5** (bug): Malformed Unicode can panic protocol handling
  - Where: `crates/dettivo-mcp/src/resources.rs:178` — `&text[i + 1..i + 3]`; `crates/dettivo-rest/src/auth.rs:23` — `auth[..7].eq_ignore_ascii_case("bearer ")`.
  - Why: Both slice UTF-8 strings at unchecked byte offsets. `resources/read` with `transcripts://search/%aé` reproducibly terminates MCP with exit 101. An authorization value such as `aaaaaaé` reaches the same defect in REST; its handler panic becomes a 500 instead of an authentication refusal.
  - Change: Decode percent escapes from bytes and inspect the authorization prefix using safe byte access. Return ordinary protocol errors for malformed input.
  - Risk: Test incomplete escapes, multibyte characters beside escapes, and non-ASCII authorization prefixes. Verify that subsequent requests still work.

- **agent-surfaces/F18** (bug): REST status labels every non-200 response as an authentication failure and exits successfully
  - Where: `crates/dettivo-cli/src/rest.rs:137` — authorization is reduced to `reply.status == 200`; `crates/dettivo-cli/src/rest.rs:182` — any listening response returns `Ok(())`; `crates/dettivo-cli/src/rest.rs:206` — all other responses print “token is refused (401)”.
  - Why: A process-hosted shim whose daemon is unavailable returns 503. Status reports that as a refused token and exits zero. Actual 401 responses also exit zero, undermining its use as a readiness check.
  - Change: Retain the actual HTTP status and error body in the report. Distinguish authentication refusal, unavailable backend and other failures, and map them to the appropriate CLI failure code.
  - Risk: Test 200, 401, 500, 503 and connection refusal against both hosting modes.

### qa-rig

- **qa-rig/F1** (bug): Inherited environment variables can redirect a scenario into real user data
  - Where: `crates/dettivo-qa/src/profile.rs:138`: `env.insert("HOME".into(), r("home"));`; `crates/dettivo-qa/src/scenarios/daemon.rs:63`: `.envs(&env)`; `crates/dettivo-core/src/paths.rs:78`: `match get(ENV_CONFIG).filter(|v| !v.is_empty())`.
  - Why: Setting private XDG paths does not remove inherited `DETTIVO_CONFIG` or `DETTIVO_DATA_DIR`, which override those paths in the product. Several daemon and driver launchers inherit the parent environment. Settings edits or model operations can therefore reach real configuration or data. `XDG_CACHE_HOME` also remains inherited.
  - Change: Use one profile command builder with `env_clear()`, an explicit desktop/session allowlist, private cache storage, and deliberate scenario overrides. Reuse the clearing pattern already present in `InsertDaemon`.
  - Risk: Display, accessibility, and audio connections require selected host variables. Test every launcher with poisoned parent overrides and verify that sentinel files outside the profile remain unchanged.

## API Contracts

<!-- API Contracts: 100% [paraphrase] -->

- No contract change unless a finding names one; a contract change is registered in `docs/api/linux-deltas.md` and its fixture updated. [paraphrase]

## Acceptance Criteria

- **R1:** daemon/F8, Transfer files rely on the caller’s umask for confidentiality: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R2:** daemon/F19, Valid request paths violate the stated log-content boundary: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R3:** agent-surfaces/F2, REST resolves a token for HTTP authentication but can omit it from IPC: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R4:** agent-surfaces/F5, Malformed Unicode can panic protocol handling: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R5:** agent-surfaces/F18, REST status labels every non-200 response as an authentication failure and exits successfully: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R6:** qa-rig/F1, Inherited environment variables can redirect a scenario into real user data: verified against the code and a test, then fixed with a regression test that fails before the fix, or rejected in the task summary with the evidence that the claim does not hold. Errors: a fix that changes a contract without its fixture and register row fails the gate. [paraphrase]
- **R7:** `just build test lint` is green at the final commit, the contract replay passes, every drive a fix touches passes under `scripts/qa/xvfb-session.sh`, ADR 0050 records the decisions this theme changed and is indexed, and the task summary lists every finding as fixed or rejected with its evidence. Errors: as stated. [paraphrase]

## Boundaries

- Fix the finding, not the neighbourhood: no new features, no refactors beyond what a finding names.
- A rejected finding is a sentence of evidence in the summary, never a silent skip.
- Files stay under the limits; a fix that would cross one splits the file.
