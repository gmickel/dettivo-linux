# Repository bootstrap: workspace, CMake skeleton and CI

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible, automated QA driving should also be first class via cua probably"
> user (turn 1): "Rust direction recorded"
> user (turn 5): "i would rather not choose the framework based on QA only, instead what is best for linux/omarchy"
> user (turn 7): "update the plan, but do deep thinking on what makes the most sense so we really know how to build this and with what"
> user (turn 8): "it should be MAXIMALLY beautiful"
> user (turn 14): "when capturing, we will be capturing specs but this will be fast work, ie it will be all no_plan specs and no review specs at the stage, we will move fast until we are done, then start cleaning up etc when all done, just as a note for M.O."
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> user (turn 17): "we will also keep adrs up to date as we go, this will be our main form of documentation"
> user (turn 17): "any and all documentation we write including adrs and user facing documentation will be done with our prose skill but will not be a list of caveats, it will be clear and what does this do for you, where is the value add"
> user (turn 13): "ALL needs to be able to be configured via config files as is the linux way"
> user (turn 20): "yes"

## Goal & Context

<!-- Goal & Context: 40% [paraphrase], 30% [strategy], 30% [inferred] -->

A contributor or an agent clones the repository, runs one command, and gets every Rust crate and every Qt binary built, linted and tested the same way CI does. This spec creates that floor so every later spec adds behaviour instead of build plumbing. [paraphrase]

The build is fast by design: specs are captured without plans or reviews during the build and a cleanup phase turns the gates on afterwards, so the bootstrap must make the cheap checks (formatting, lints, unit tests, documentation) automatic from the first commit while leaving the expensive gates configurable and off. [paraphrase]

Documentation for this repository is the set of architecture decision records plus user-facing pages, all written value-first; the bootstrap puts the practice in the contribution guide so it is visible before the first pull request. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 60% [paraphrase], 40% [inferred] -->

- One Rust workspace holding one crate per component named in ADR 0001 and ADR 0003: the contract types, the domain core, audio, the engine protocol, the four engine binaries, speech and language provider layers, insertion, storage, the daemon, the CLI, the MCP server and the QA runner. Each crate starts empty with a compiling smoke test and its declared dependency edges, so later specs fill bodies rather than create crates. [paraphrase]
- One CMake project for the Qt binaries (the app, the OSD host and the insertion test target) and the shared QML module, built against Qt 6.8 or newer per ADR 0013, with the module installed to the system QML location. [paraphrase]
- One build entry point, a `just` recipe set, that fans out to Cargo and CMake for build, test, lint, QA and package, so no contributor needs to know which toolchain owns which target. [inferred]
- A CI definition that runs in a container carrying Rust, Qt 6, CMake and the Vulkan build dependencies (headers and shader compiler) named in ADR 0004, with separate jobs for formatting, clippy, Rust unit tests, QML lint and a documentation check. [paraphrase]
- flow-next configuration with the QA pipeline stage off and no review backend, plus a feature map seeded from the design coverage table, so autonomous runs follow the fast-build way of working from the start. [paraphrase]

## API Contracts

<!-- API Contracts: 50% [paraphrase], 50% [inferred] -->

- `just build`, `just test`, `just lint`, `just qa`, `just package` are the only documented entry points; each exits non-zero on the first failing toolchain and prints which one failed. [inferred]
- CI job names are stable identifiers other specs reference: `fmt`, `clippy`, `unit-rust`, `unit-qml`, `docs`. [inferred]
- The workspace and CMake project expose the crate and binary names decided in ADR 0001 (`dettivod`, `dettivo`, `dettivo-app`, `dettivo-osd`, `dettivo-engine-whisper`, `dettivo-engine-parakeet`, `dettivo-engine-llm`, `dettivo-engine-diarize`). [paraphrase]

## Edge Cases & Constraints

- A machine without Qt 6.8 or without the Vulkan build dependencies fails the build with a message naming the missing dependency; nothing is skipped silently. [inferred]
- The CI container is pinned to an image digest so a toolchain update is a deliberate change, never a surprise. [inferred]
- The bootstrap adds no product behaviour, no engine code, no model downloads and no desktop drive; those are the specs that follow. [paraphrase]
- Files stay under roughly 500 lines for Rust and C++ and 300 for QML, and the lint enforces the limit from the start. [paraphrase]

## Acceptance Criteria

- **R1:** A fresh clone builds every Rust crate and every Qt binary from a single `just build` and passes `just test` and `just lint`, and CI runs the same recipes. Errors: a missing toolchain or dependency fails the recipe with the dependency named; a partial build never reports success. [paraphrase]
- **R2:** The Rust workspace contains one crate per component named in ADR 0001 and ADR 0003, each with a compiling smoke test and its dependency edges declared, and no crate depends on a crate it should not (the QA runner and the CLI never depend on the daemon's internals). Errors: a cycle or a forbidden edge fails `just lint`. [inferred]
- **R3:** The Qt binaries build with CMake against Qt 6.8 or newer and install the shared QML module to the system QML location; a Qt below 6.8 fails configuration with the version named. Errors: no error surface beyond the version check. [inferred]
- **R4:** CI runs `fmt`, `clippy`, `unit-rust`, `unit-qml` and `docs` on every push in a pinned container with Rust, Qt 6, CMake, Vulkan headers and the shader compiler. Errors: any job failure blocks merge; a container pull failure fails the run rather than falling back to the host. [paraphrase]
- **R5:** flow-next configuration has the QA pipeline stage off and no review backend, and a feature map exists with one entry per surface in the design coverage table. Errors: no error surface beyond flow-next's own validation. [user] [strategy:Complete speech workflows, proven by drives]
- **R6:** The contribution guide states that architecture decision records are the documentation, that every document leads with what it does for the reader, and how to add a record from the template. Errors: no error surface. [paraphrase]
- **R7:** The file-length lint fails on Rust or C++ files over 500 lines and QML files over 300 lines, with an allowlist for generated code. Errors: an unlisted long file fails `just lint`. [paraphrase]

## Boundaries

- No daemon, engine, insertion or UI behaviour ships in this spec; every crate and binary is a compiling shell. [paraphrase]
- No plan review, implementation review or QA pipeline stage is enabled; the cleanup phase after v1 owns that. [user]
- No Flatpak or Snap packaging, and no CUDA build variant. [paraphrase]
- No model download or catalogue work. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- Speed of the whole build is the priority: specs are captured with `--no-plan`, worked directly, and reviewed in a cleanup phase after v1 is complete, so the bootstrap must make the cheap gates automatic and keep the expensive ones switchable. [user]
- The repository is the only public artefact of the product plan; decision records under the docs tree and the strategy anchor are what a reader gets, so the bootstrap puts the documentation practice in front of contributors immediately. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the QA runner crate and the CI job names exist from day one so every later workflow spec has an evidence route to plug into. [strategy:Complete speech workflows, proven by drives]
- **Omarchy-native, beautiful by default:** the QML module and its install location are created here so the design system spec can start the moment the workspace exists. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD — populate via /flow-next:plan or the no-plan route) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
