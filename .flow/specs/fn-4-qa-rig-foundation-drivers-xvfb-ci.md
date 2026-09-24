# QA rig foundation: drivers, Xvfb CI, virtual audio and QA mode

## Conversation Evidence

> user (turn 1): "automated QA driving should also be first class via cua probably"
> user (turn 1): "to get this developed as quickly as possible"
> user (turn 5): "i would rather not choose the framework based on QA only, instead what is best for linux/omarchy"
> user (turn 14): "it will be all no_plan specs and no review specs at the stage, we will move fast until we are done, then start cleaning up etc when all done"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> user (turn 20): "yes"

## Goal & Context

<!-- Goal & Context: 40% [user], 30% [paraphrase], 30% [strategy] -->

Every user-facing behaviour gets an automated way to prove it works on a real desktop, and that proof runs in CI without a microphone or a person at the keyboard. This spec builds the rig the later workflow specs plug scenarios into: a driver that can find and act on real windows through the accessibility tree, a headless display in CI, a way to feed real audio into the real capture path, and the switches inside the product that make runs deterministic. [paraphrase]

Automated QA driving through cua-driver is first class by the user's direction. The probe on the development machine showed cua-driver 0.20 lists and drives only X11 windows on Hyprland and injects input through XTEST, so the Qt binaries run under the X11 platform plugin in QA and an in-repo fallback driver exists from day one, because the Windows port lost weeks when its only harness broke. [user]

## Architecture & Data Models

<!-- Architecture & Data Models: 50% [paraphrase], 50% [inferred] -->

- The QA runner crate defines a driver interface (launch, find element by accessible name, click, type, read value, wait for label, screenshot) with two implementations: cua-driver through its command-line tool, and an in-repo AT-SPI driver with XTEST key injection that depends on nothing outside the repository. Scenarios are written against the interface and can run on either. [paraphrase]
- Every scenario runs in an isolated profile: its own data, config, state and runtime directories, with the real model directory linked in so nothing is downloaded twice. Cleanup is scoped to the scenario root and fails the run if anything is left behind. [paraphrase]
- An evidence directory per run collects screenshots, accessibility snapshots, trajectories and the runner's JSON receipt; the flow-next QA pass and the release gate read it. [inferred]
- A CI job runs the drives on a headless X display with a minimal window manager, a session bus and the accessibility bus, in the same pinned container the bootstrap defined. A placeholder Qt window from the app skeleton carries an accessible label the first scenario asserts on both drivers. [paraphrase]
- The virtual audio rig is a script set: it creates null sinks, plays fixture audio into them on cue, exposes their monitors as inputs, and can unload and reload a sink mid-run to simulate a device swap. A recorder verifies the round trip. [paraphrase]
- QA mode inside the product: the daemon and the app read the QA environment variables (the macOS-compatible mock switches, fixture microphone and system audio, mock insertion sink, mock language model, fixture model server, seeded history, route opening, isolated locations, forced CPU) and refuse QA mode in release builds unless explicitly allowed. [paraphrase]
- The contract replay harness drives a live daemon with the fixture suite from the contract spec and diffs responses with tolerant matching for ids and timestamps. [paraphrase]
- Three lints run in CI: every interactive control has an accessible name, no release route contains developer text (exceptions, raw paths, model file names, internal identifiers), and Qt Quick Tests run through the QML test runner. [paraphrase]

## API Contracts

<!-- API Contracts: 60% [paraphrase], 40% [inferred] -->

- `just qa` runs the contract replay, the Xvfb drive pack and the lints; `just qa drive <scenario> --driver cua|atspi` runs one scenario on one driver. [inferred]
- QA environment variables and their effects are documented in one table and covered by unit tests in the daemon and app; unknown QA variables are rejected with the name printed. [paraphrase]
- The rig scripts accept a fixture path and a sink name and print the monitor source name they created. [inferred]
- The runner's receipt is JSON with scenario id, driver, outcome, evidence paths and timings. [inferred]

## Edge Cases & Constraints

- cua-driver is pinned per release; its Wayland support is re-checked at every upgrade, and the drive moves to Wayland-native windows only when a probe shows it works. [paraphrase]
- The bar widget has no accessibility tree, so its scenario asserts daemon state through the CLI and compares a screenshot region with tolerance. [paraphrase]
- Harness load is measured with the harness idle before any CPU claim is attributed to the product. [paraphrase]
- If PipeWire's dummy driver proves unstable in CI, the rig falls back to the fixture microphone variable and runs for real only on the development machine. [paraphrase]

## Acceptance Criteria

- **R1:** The CI drive job launches the placeholder window on a headless X display and asserts its accessible label with both the cua-driver and the in-repo driver, and writes an evidence directory with a screenshot and a JSON receipt. Errors: a missing display, session bus or accessibility bus fails the job with the missing piece named. [paraphrase]
- **R2:** Every scenario declares its driver interface calls only; the same scenario file runs on either driver, and a weekly CI job runs the whole pack on the fallback driver. Errors: a scenario calling a driver-specific API fails a lint. [paraphrase]
- **R3:** Every scenario runs in an isolated profile with the model directory linked in, and the runner fails when files, sockets or processes are left outside the scenario root. Errors: leaked artefacts are listed by path. [paraphrase]
- **R4:** The virtual audio rig creates a null sink, plays a fixture into it, and a recorder reading the monitor reproduces the fixture within a stated tolerance; unloading the sink mid-recording is detectable by the recorder. Errors: rig creation failure names the PipeWire module that failed. [paraphrase]
- **R5:** The daemon and the app honour every documented QA environment variable, refuse QA mode in release builds unless explicitly allowed, and each variable has a unit test. Errors: an unknown QA variable is rejected with its name. [paraphrase]
- **R6:** The contract replay harness runs the fixture suite against a live daemon in CI and reports per-method pass or fail with tolerant matching for ids and timestamps. Errors: a fixture whose capability flag the daemon does not declare is skipped and reported, never counted as passed. [paraphrase]
- **R7:** The accessible-name lint, the negative text scan and the Qt Quick Test runner run in CI and fail on a violation, with the offending control, text or test named. Errors: no error surface beyond the failures themselves. [paraphrase]

## Boundaries

- No product scenario beyond the placeholder window; dictation, GUI and meeting packs are their own specs. [paraphrase]
- No visual regression baselines or frame-timing capture yet; that is the visual regression spec. [paraphrase]
- No flow-next QA pipeline stage during the build; the rig exists so it can be turned on in the cleanup phase. [user]

## Decision Context

### Motivation
<!-- scope: business -->

- The user set automated QA driving as first class; the fast-build way of working keeps the flow-next QA stage off, so the rig must be cheap to run from CI and from a single command without a review loop around it. [user]
- A second driver that depends on nothing outside the repository is insurance the Windows port did not have. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** this is the drive machinery the track relies on. [strategy:Complete speech workflows, proven by drives]
- **Contract parity and agent surfaces:** the replay harness is how conformance is proven against a live socket. [strategy:Contract parity and agent surfaces]

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
