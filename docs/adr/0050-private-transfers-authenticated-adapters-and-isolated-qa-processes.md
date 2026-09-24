# 0050. Transfers stay private, adapters preserve authentication and QA processes use isolated paths

Status: Accepted 2026-09-10

## What this gives you

Audio uploads and transcript exports remain private when the daemon runs under a permissive umask. REST clients use the same resolved token on HTTP and IPC, malformed Unicode produces a protocol error, and `dettivo rest status` fails when the listener cannot serve an authenticated request. QA processes cannot inherit configuration or data overrides that redirect a scenario into the user's files.

## Situation

The six findings in fn-48 held against the current source. Transfer creation relied on the caller's umask, two successful operations logged request values, process-hosted REST could drop a resolved token before IPC, byte offsets could split UTF-8 in MCP and REST, REST status mislabeled backend failures as token refusals, and several QA launchers inherited product path overrides. Focused regression tests reproduce each defect before its fix.

This record amends [0011](0011-qa-drives-and-audio-rig.md), [0019](0019-mcp-hand-rolled-protocol-and-tool-mapping.md), [0022](0022-chunked-import-symphonia-decoder-and-overlap-merger.md), [0028](0028-rest-shim-hand-rolled-server-and-status-map.md) and [0043](0043-qa-profiles-carry-a-private-models-directory-and-the-gate-passes-on-what-the-code-does.md).

## Decision

- The daemon creates transfer directories with mode `0700` and transfer files with mode `0600`, and corrects the permissions of existing transfer directories. Exclusive file creation protects pre-existing files from truncation. Upload, export and expiry keep their existing protocol shapes. This closes daemon/F8.
- Cancellation and endpoint-trust logs retain their operation names without the client cancellation reason or endpoint. The trace-level marker test exercises both successful operations. This closes daemon/F19.
- REST resolves one shared token and passes it to the socket client used for settings and process-hosted requests. Environment, Secret Service and token-file resolution retain their precedence. This closes agent-surfaces/F2.
- MCP percent decoding operates on bytes and rejects incomplete escapes, invalid hex pairs and invalid decoded UTF-8 with `INVALID_PARAMS`. REST checks the bearer prefix without slicing a UTF-8 string at unchecked offsets. A malformed request leaves subsequent requests usable. This closes agent-surfaces/F5.
- REST status retains the HTTP status and error body. HTTP `200` exits `0`, authentication refusal exits `3`, an unavailable backend or refused connection exits `2`, and other HTTP failures exit `1`. Human output names the observed failure and JSON keeps the structured report. This closes agent-surfaces/F18.
- Profile-backed QA launches clear their inherited environment, forward an explicit desktop/session/audio allowlist and apply the scenario's deliberate overrides. Profiles provide private home, configuration, state, data and cache paths. The shared command builder serves daemon, CLI and GUI launchers. This closes qa-rig/F1.

## Consequences

Existing transfer files remain behind a private directory; a restarted daemon does not overwrite an occupied transfer filename. Token values and request content remain absent from the new diagnostics. Scripts using `rest status` as a readiness check now receive failure exits when the listener or backend is unavailable.

The MCP malformed-escape behavior and REST status report are registered in [the Linux delta register](../api/linux-deltas.md). Regression tests cover permissive file creation, existing directories, successful log-marker requests, token sources, malformed Unicode with a subsequent request, the status matrix and poisoned QA parent environments. The task evidence records the exact gate and drive outcomes.

Contract verification exposed an inherited matcher defect and stale transcript examples. The replay now compares only documented machine/store subtrees by shape and compares the remaining envelope, errors and fixed capabilities exactly. The transcript fixtures retain their original examples and add the meeting timeline and dictation search fields already registered by ADRs 0038 and 0025. A separate verification repair carries these changes; it changes no product response.

The OSD drive compares its displayed outcome and reason with the stopped take's persisted insertion facts, including a valid `target_is_self` refusal. The import drive observes dialog dismissal before reopening it. Settings identifies segmented-control tabs inside the container's bounds when a driver appends the container after its action tokens. These checks retain their failure assertions while avoiding guessed refusal text, clicks during a closing popup and reliance on snapshot ordering.

QA still needs selected host connections for display, accessibility and audio. Those connections cross the profile boundary deliberately; configuration, cache and model storage do not. A scenario may override the profile environment explicitly. Engine benchmark commands that use explicit input/model paths retain their hardware environment, and the opt-in live Omarchy check retains its live-session semantics.
