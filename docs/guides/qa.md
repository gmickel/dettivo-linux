# QA guide

Every behaviour the product promises has an automated route that proves it on a real desktop, and a release is the run of every route with its blockers named. This guide is the path through the rig for someone who has to prove a change or ship a build: the rig itself, the packs, the visual matrix, the benchmarks, the evidence map that ties every requirement to a route, the release gate that runs them all, and how to write a scenario. Each section links the page that holds the details ([docs/qa.md](../qa.md) is the reference).

## The rig

A drive finds and acts on real windows through the accessibility tree on one of two drivers (`cua-driver`, and the in-repo `atspi` fallback), every scenario runs in an isolated profile with the real model directory linked in, and the evidence (screenshot, tree, receipt, logs) lands under `qa-evidence/<run>/<scenario>.<driver>/` ([docs/qa.md](../qa.md#running-it)). `dettivo-qa doctor` names what a desktop is missing; `scripts/qa/xvfb-session.sh <command>` wraps a command in Xvfb, a session bus, the accessibility bus and a window manager, which is how CI runs and how a desktop keeps a drive off its live screen. Audio goes through the virtual PipeWire rig ([docs/qa.md](../qa.md#virtual-audio)) and the product's own QA switches make a run deterministic ([docs/qa.md](../qa.md#qa-mode-inside-the-product)).

```
just qa                         # the contract replay, the MCP and REST harnesses, the lints, the drive pack
just qa-drive <scenario> atspi  # one scenario on one driver
dettivo-qa list                 # every scenario with its summary
```

## The packs

A pack runs scenarios and repository commands in one order into one report, `<pack>-pack.json` for a machine and `<pack>-pack.md` for a person, with every step's outcome, the measurements against the NFR targets and a blockers list naming every step that could not run ([ADR 0017](../adr/0017-qa-packs-and-the-dictation-report.md)). `dettivo-qa pack list` names them:

| Pack | Proves | Page |
|---|---|---|
| `dictation` | Hotkeys, the pill on both drivers, the insertion matrix, the self-insertion refusal, the history round trip, the first-insert timing and the Whisper WER | [docs/qa.md](../qa.md#the-dictation-pack) |
| `gui` (and `gui-onboarding`, `gui-settings`, `gui-history`, `gui-omarchy`) | The four GUI surfaces on both drivers, every route scanned for developer text and unnamed controls | [docs/qa.md](../qa.md#the-gui-pack) |
| `meetings` | The rig capture, recovery, the live transcript, diarization, the exports, the three screens, the token fixture, the throughput figures and the GPU proof | [docs/qa.md](../qa.md#the-meetings-pack) |
| `release` | The whole release gate, every step below with its report embedded | [the release gate](#the-release-gate) |

## Visual

`just qa-visual` renders every surface of `qa/visual/manifest.toml` at 1x and 2x on three theme fixtures and both built-in palettes and diffs each against its approved baseline; `just qa-visual-canary` renders with a deliberate token regression and passes only when every diff fails; `just qa-visual-approve <surface>` makes a fresh render the baseline and records it in [docs/design/baselines.md](../design/baselines.md) ([docs/qa.md](../qa.md#visual-regression), [ADR 0021](../adr/0021-visual-regression-gate-and-frame-pacing.md)). The render pass carries the negative style check, so a stock control or a fallback font fails by name.

## Benchmarks

`just bench <dir>` and `just bench-cpu` measure the NFRs (first insert, transcription throughput, the language model, the idle footprint, startup) from the fixtures the tests use and file one report per host and tier under [docs/reports/benchmarks/](../reports/benchmarks/README.md), every figure beside its calibrated target; the meetings pack's `--record` adds the meeting and diarization factors to the same report ([docs/qa.md](../qa.md#benchmarks), [ADR 0029](../adr/0029-nfr-calibration-and-the-benchmark-suite.md)). A missed target is recorded and quoted in the release notes, never gated.

## The evidence map

`dettivo-qa evidence-map` proves that every requirement of every v1 spec has a route that exists (an inventory of routes, not their results; the release gate runs them): `qa/evidence-map.toml` lists, per spec under `.flow/specs/` and per R-ID, one or more routes, and the verb reads every spec's `## Acceptance Criteria`, checks that each R-ID has a route, resolves every `ref` against the repository (a `unit` ref must carry `#[test]` or `#[tokio::test]`, a `visual` ref may name a state as `surface/state`, a `human` ref is a receipt file, never proof that a person walked it), and writes [docs/reports/evidence-map.json](../reports/evidence-map.json) with its Markdown twin. Coverage under 1.0, an R-ID the map names that the spec does not have, or a `ref` that does not resolve exits 1 naming the spec, the R-ID, the kind and the reference; `--write` refreshes the checked-in report and `just qa-evidence-map` runs it.

```toml
[[spec]]
id = "fn-9-dictation-session-state-machine-ipc"

[[spec.route]]
rids = ["R1", "R2"]          # one route may serve several R-IDs
kind = "drive"               # unit | contract | pipeline | drive | visual | pack | bench | docs | script | human
ref = "osd_dictation"        # what the kind resolves: see the table
note = "the session's transitions on the event stream through a mock-microphone dictation"
```

| Kind | `ref` | Resolved against |
|---|---|---|
| `unit` | `<crate>::<test function>` or `qt::<test name>` | `fn <name>(` under `crates/<crate>` or `tools/<crate>`; `function <name>(`, `void <name>()` or `add_test(NAME <name>` under `qt/` |
| `contract` | a fixture file or directory | `crates/dettivo-proto/fixtures/`, `crates/dettivo-rest/fixtures/` |
| `pipeline` | `parakeet-alignment`, `import-merge`, `diarization` | the pipelines `dettivo-qa pipeline` runs |
| `drive` | a scenario id | `dettivo-qa list`, plus `settings_roundtrip.<section>` |
| `visual` | a surface name | `qa/visual/manifest.toml` |
| `pack` | `<pack>/<step>` | `dettivo-qa pack list` and the release gate's steps |
| `bench` | a step name | the benchmark steps and the meetings rows |
| `docs` | a path under `docs/` | the file exists |
| `script` | a path under `scripts/` | the file exists and is executable |
| `human` | a receipt path | the file exists; listed separately in the report as a route only a person walks |

The report carries `schema_version`, `git_sha`, `specs` (one block per spec with `rids`, `mapped`, `coverage`, the routes with their resolution) and the totals `rids`, `mapped`, `coverage`, `human_routes` and `unresolved`.

## The release gate

`dettivo-qa pack release` is the release gate as one script with named steps and JSON output ([docs/RELEASING.md](../RELEASING.md)): it runs every step below in order, embeds each step's own report under `steps[].report`, and ends with `blockers` (every step that skipped or failed, with its reason) and `external_blockers` (the subset that names something outside this machine and this repository: the CPU-only VM, a driver tool that is not installed, a row that fails on `cua-driver` and passes on the in-repo driver, a workflow GitHub has disabled, the live desktop an Xvfb session cannot reach, a person at the microphone). The claim "only external blockers remain" is true when `passed` is `true` and every blocker is in `external_blockers`; a blocker outside that list fails the gate. `--skip <step>` leaves a step out, recorded as such; `--cpu`, `--engines <dir>` and `--driver` reach the packs it runs; `--record` files the run as `docs/reports/release-gate/<version>.json` with its `.md`, the artifact the release quotes. `just qa-release` runs it.

| Step | Runs | Passes when |
|---|---|---|
| `version_match` | `Cargo.toml`, `CHANGELOG.md`, `git tag` | the changelog has the version's section and any `v<version>` tag points at this commit |
| `ci_green` | `gh run list --commit <sha>` | every workflow run for the commit concluded `success`; no run at all is an external blocker naming the disabled workflows |
| `contract_strict` | `dettivo-qa contract --strict` | every fixture passed or skipped; a pending method is allowed only behind a capability flag the daemon declares `false` |
| `mcp_harness` | `dettivo-qa mcp` | every harness step passed in both framings |
| `pack_dictation`, `pack_gui`, `pack_meetings` | `dettivo-qa pack <name> --continue` | every row passed or skipped for an allowed reason; the pack's blockers join the gate's list, and a row that fails on `cua-driver` while its `atspi` twin passes is an external blocker naming the driver rather than a failed step |
| `bench_report` | `docs/reports/benchmarks/` | a report exists for this host on the `gpu` and the `cpu` tier, each from a commit in this history; a CPU row measured under `DETTIVO_FORCE_CPU=1` names the CPU-only VM as an external blocker |
| `install_test` | `install-test.json` (`DETTIVO_INSTALL_TEST_REPORT`, else `build/install-test.json`) | every check passed; the `--session` run on a fresh VM is an external blocker until that report is the one read |
| `evidence_map` | `dettivo-qa evidence-map` | coverage 1.0, every ref resolved |
| `docs_build` | `scripts/check-docs.sh` | no finding |
| `notice_lint` | `cargo run -p xtask -- lint-notice` | every crate has its row and every reuse marker has its row |
| `real_headset` | `dettivo-qa audio-check` and the receipt `docs/reports/release-gate/real-headset.json` | the rig round trip passed and a person recorded one dictation through the real microphone; without the receipt, an external blocker naming what the person does |

The report carries `schema_version`, `version`, `git_sha`, `machine`, `started_unix`, `duration_ms`, `passed`, `steps` (`id`, `outcome`, `duration_ms`, `command`, `exit_code`, `reason`, `evidence`, `report`), `blockers` and `external_blockers` (`step`, `reason`, `needs`).

## Writing a scenario

A scenario is a Rust type under `crates/dettivo-qa/src/scenarios/` that implements `Scenario`: an `id`, a `summary`, an optional `preflight` naming what the desktop must have, `needs_driver` (false for a scenario that drives the socket and the compositor alone), `preconditions` (a model, a binary) and `run`, which drives through the `Driver` interface only (launch, find by accessible name, click, type, read, wait, screenshot) and writes its evidence through the context. Register it in `scenarios::all()`, give every control it touches a documented accessible name ([docs/qa/a11y-names.md](../qa/a11y-names.md)), add it to the pack that proves its slice, and add its route to `qa/evidence-map.toml`. `dettivo-qa lint-scenarios` fails a scenario that names a driver implementation ([docs/qa.md](../qa.md#drivers)); `just qa-drive <id> atspi` and `just qa-drive <id> cua` run it on both drivers.
