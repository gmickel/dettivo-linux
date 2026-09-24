# Contract import: IPC v1 types, delta register and fixture suite

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible, automated QA driving should also be first class via cua probably"
> user (turn 1): "Dictation, meeting, Omarchy bar/GUI and CLI/MCP discovery collated"
> user (turn 1): "Voxtype meeting CLI documented"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> user (turn 17): "we will also keep adrs up to date as we go, this will be our main form of documentation"
> user (turn 20): "yes"

## Goal & Context

<!-- Goal & Context: 30% [paraphrase], 40% [strategy], 30% [inferred] -->

A skill, script or MCP client written against Dettivo on macOS works against Dettivo on Linux with the same method names, tool names, resource URIs, error codes and exit codes. This spec makes that promise checkable before any server exists: the contract documents come into the repository pinned to the commit they were copied from, the Rust types that every Linux surface will share are written from them, every difference Linux intends is registered with a reason, and a fixture suite states what conformance means. [strategy:Contract parity and agent surfaces]

The macOS repository owns the contract and the Windows port grew far past it without a parity document, then reconstructed its drift by hand. The delta register with a disposition per contract item is what stops that happening a third time. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- The two macOS contract documents (IPC v1 and the REST shim v1) are copied unchanged into the repository's API documentation with a header naming the macOS commit they came from. [paraphrase]
- A contract-types crate holds the JSON-RPC 2.0 envelope, every implemented method's params and result, the error taxonomy with its numeric codes and app codes, the event notification shape and topics, the capability flags, and the shared runtime types (`TranscriptRef`, `HistoryItem`, `JobStatus`, `InsertionResult`). The types are plain data with serde derives and no behaviour, so the daemon, the CLI, the MCP server and the QA runner depend on one definition. [paraphrase]
- A delta register lists every Linux addition, omission and behavioural difference with its reason and the capability flag that signals it, and gives every contract item a disposition: implemented, covered elsewhere, deferred, blocked. Adopted from the Windows port: the `speech.providers.list` and `speech.selection.get|set` method names and the `acknowledge_meeting_disclosure` parameter. [paraphrase]
- A golden fixture suite: one directory per namespace, one file per method and error case holding the request, the expected response or error, and the capability flags it needs. A round-trip test proves every fixture deserialises and re-serialises byte-stable through the contract types. The socket replay harness that drives a live daemon with these fixtures is built by the QA rig spec, on the same files. [inferred]
- Reserved namespaces and methods (`knowledge.*`, `meeting.templates.*`, `retention.*`, `meetings.delete`, `automation.jobs.*`, `automation.providers.*`) are enumerated in the types with the reserved shapes so a router can answer `NOT_IMPLEMENTED` with the right body. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Transport framing is newline-delimited JSON-RPC 2.0, one message per line in both directions. [paraphrase]
- Error codes: `INVALID_PARAMS` -32010, `UNAUTHORIZED_CLIENT` -32011, `NOT_FOUND` -32012, `CONFLICT` -32013, `NOT_IMPLEMENTED` -32014, `INTERNAL_ERROR` -32015, `RATE_LIMITED_LOCAL`; `APP_NOT_RUNNING` exists for adapters only. Error `data` carries `app_code`, `retryable` and `details` with the domain and the Dettivo error code and kind. [paraphrase]
- `system.capabilities` carries the macOS flag set (`auth`, `formats`, `insert_modes`, `transfers`, `automation`, `knowledge`, `meeting_templates`, `retention`) plus two Linux additions registered as deltas: a `platform` block (os, transport, compositor, session, insertion backend, gpu) and a `speech` block naming the adopted Windows methods. [paraphrase]
- Server notification `events.notify` with `subscription_id`, `topic`, `timestamp`, `payload`; topics `dictation.state`, `meeting.state`, `job.progress`, `events.overflow`, plus the Linux additions `audio.level`, `model.download`, `engine.state`, `meeting.segment` registered as deltas. [paraphrase]
- Meeting export formats declare `json` alongside `txt`, `md`, `srt` and `vtt`, registered as a delta because the macOS export service produces it while its capability list omits it. [paraphrase]

## Edge Cases & Constraints

- The copied contract documents are never edited in place; corrections are proposed upstream and the copy is refreshed with a new commit pin. [inferred]
- A fixture without a capability-flag annotation fails the suite's own lint, so conformance is always conditional on declared capabilities. [inferred]
- Ids are lowercase UUID strings everywhere; the types reject anything else at deserialisation. [paraphrase]
- The fixture format is plain JSON so a macOS or Windows harness can consume it later without this crate. [inferred]

## Acceptance Criteria

- **R1:** The IPC v1 and REST v1 documents are present in the repository byte-identical to the macOS source at a named commit, with the pin recorded in the delta register. Errors: no error surface beyond a mismatch check in the docs job. [paraphrase]
- **R2:** The contract-types crate round-trips every fixture in the suite byte-stable through serialisation, for every implemented method, every error case and the capability snapshot. Errors: an unknown field, a non-UUID id or a code outside the taxonomy fails deserialisation with the field named. [paraphrase]
- **R3:** The error taxonomy exposes the seven server codes with their numeric values and the adapter-only code, and a helper builds the `data` block with app code, retryability and details. Errors: no error surface beyond R2. [paraphrase]
- **R4:** The capability type carries the macOS flags and the Linux `platform` and `speech` additions, and the delta register names both additions with their reason. Errors: a capability fixture missing a required flag fails the suite. [paraphrase]
- **R5:** The delta register gives every method, resource, tool, export format and event topic in the copied contract one disposition from the fixed set, and a lint fails when a contract item has none. Errors: unknown disposition value fails the lint. [paraphrase]
- **R6:** Reserved namespaces and methods are enumerated with their reserved shapes, and a fixture per reserved method expects `NOT_IMPLEMENTED`. Errors: no error surface beyond R2. [paraphrase]
- **R7:** The fixture suite runs in CI as part of the Rust unit job and its layout is documented so the QA rig can replay the same files against a live socket. Errors: a fixture directory without a namespace or method name fails the suite. [inferred]

## Boundaries

- No socket server, router or authentication ships here; the daemon skeleton spec owns those. [paraphrase]
- No CLI, MCP or REST code; only the types and documents they will share. [paraphrase]
- No Linux-only method is implemented; additions are registered, not built. [inferred]

## Decision Context

Copying the contract verbatim with a commit pin, rather than rewriting it for Linux, keeps one source of truth on macOS and makes drift measurable. The Windows method names for provider selection are adopted because two ports already agree on them and inventing a third vocabulary would cost every agent author. The fixture format is plain JSON so it outlives this crate and can be pointed at the other ports. [paraphrase]

## Strategy Alignment

- **Contract parity and agent surfaces:** this spec is the track's foundation; honest capability flags and the fixture suite are the mechanisms the track names. [strategy:Contract parity and agent surfaces]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
