# QA pack: the dictation vertical slice, timing and the release-shaped report

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-12 row: "`qa-pack-dictation-vertical-slice` | phase 1 | depends on S-03, S-08, S-09, S-11 | NFR-1, NFR-3 first measurement; 9.7 | Full dictation drive on Thor and CI; matrix report artifact"
> masterplan milestone: "First dogfood is S-12 green on Thor: hold F9 in ghostty and text lands."
> masterplan phase 1 exit criteria: "Whisper engine WER fixture passes on CPU and Vulkan; insertion matrix passes for terminal, Qt target and Chromium; hotkey scenario on Thor passes; history persisted and searchable; design tokens resolve from three Omarchy themes; OSD baseline approved and frame pacing green"
> masterplan NFR-1: "Time to first insert, warm engine, GPU tier | p50 ≤ 1.0 s for ≤ 15 s audio with Parakeet or Whisper small | QA rig timing on Thor"
> masterplan NFR-3: "Insertion reliability | ≥ 98 % across the target app matrix (section 9.7) | Insertion matrix pack"
> masterplan 9.7: "Each cell records backend used, latency, and pass or fail. The matrix report is a release artifact."
> masterplan QA-9: "The release gate is one script with named steps and JSON output; the "only external blockers remain" claim must name them."
> masterplan QA-10: "Drives record trajectories and screenshots into an evidence directory that the flow-next QA pass and the release gate consume."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

The first dogfood becomes a pack that proves itself. `dettivo-qa pack dictation` runs the whole slice on one machine, in order: the hotkey press through the compositor binding, the pill's states, the Whisper transcription, the insertion into every target the machine offers, the item in the history store and its re-insertion after a daemon restart; it times the path from key release to text visible for NFR-1 and counts insertion outcomes across the matrix for NFR-3; and it writes one report, `dictation-pack.json` with a human `dictation-pack.md`, that names every step, its evidence directory, its measurement and whatever external blocker kept a step from running. The same pack runs under Xvfb in CI with the fixture microphone and the targets a container has, and on the development machine under Hyprland with the real chords, terminals and Chromium. It is the phase 1 exit check and the first release-gate step. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `dettivo-qa pack <name>` (a new verb beside `drive`): a `Pack` is an ordered list of steps, each a scenario id plus the driver, the profile isolation it needs, an expected outcome (`pass`, `skip-allowed` with the reason class), and the measurements it contributes; the pack runs steps in one evidence run directory (`qa-evidence/<run>/pack-dictation/`), stops on a hard failure unless `--continue`, and writes the report. Existing scenarios stay unchanged; the pack composes them. [inferred]
- The dictation pack's steps: `hotkeys_hyprland` (skipped with the reason where Hyprland or uinput is absent), `osd_dictation` (both drivers), `insertion_matrix` (every target the machine has), `never_into_self`, a new `history_roundtrip` scenario (dictate through the fixture microphone, find the item with `transcripts.search`, restart the daemon, `dictation.reinsert_last`, read the text back), and a new `first_insert_timing` scenario (ten warm dictations of the 11 s fixture into the Qt target; the daemon's event stream timestamps give release-to-inserted per run; p50 and p95 land in the report against the NFR-1 target for the machine's tier). [paraphrase]
- Timing source: `dictation.state` events already carry timestamps; the scenario records `stop` request time and the `inserted` completion event; the engine's own transcription time is read from the completion payload's `timings` block (Linux addition to `dictation.state`: `capture_ms`, `transcribe_ms`, `insert_ms`), so the report can split the path. [inferred]
- Reliability: the matrix report's rows are aggregated into `insertion_reliability` (passes over attempted targets; skipped targets are excluded and listed) with the NFR-3 target beside it. [paraphrase]
- Report: `dettivo-qa pack --json` writes `dictation-pack.json` (`{ machine, tier, steps: [{ id, driver, outcome, duration_ms, evidence, reason }], measurements: { first_insert_p50_ms, first_insert_p95_ms, nfr1_target_ms, insertion_reliability, nfr3_target, matrix: [...] }, blockers: [...] }`) and renders `dictation-pack.md`; `blockers` names each step that could not run and why (a missing tool, no display, no GPU), which is the "only external blockers remain" list QA-9 asks for. Machine and tier come from `system.capabilities` and the engine's backend. [inferred]
- CI: the drive job runs `just qa-pack dictation` after the individual drives, uploads the report with the evidence, and fails on a hard failure; on CI the NFR numbers are recorded, never gated (the targets are Thor targets), and the report says so. On the development machine `just qa-pack dictation` is the phase 1 exit check; `docs/qa.md` gains the pack section and `docs/RELEASING.md` is started with the pack as its first named gate step. [inferred]
- The whisper engine's WER fixture (from the engine spec) is a pack step on CPU in CI and on Vulkan on the development machine, reported with its threshold. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo-qa pack dictation [--driver atspi|cua] [--continue] [--json] [--out <dir>]`; `dettivo-qa pack list` names the packs and their steps. Exit 0 when every step passed or was skipped for an allowed reason, 1 on a hard failure, 2 on a preflight refusal. [inferred]
- `dictation.state` completion payload gains `timings` (Linux addition, optional, recorded in `docs/api/linux-deltas.md` with fixtures). [inferred]
- `just qa-pack <name>` and the CI drive job step; the report files are named in `docs/qa.md`. [inferred]
- No new daemon methods. [inferred]

## Edge Cases & Constraints

- A step's scenario is skipped for an allowed reason (no Hyprland on CI, no Chromium): the pack records it as skipped with the reason and continues; the reliability figure excludes it. [paraphrase]
- A hard failure mid-pack: the report still writes with the failed step and every step after it marked `not_run`, so the evidence is never lost. [inferred]
- Timing on a cold engine: the first run warms the engine and is excluded from the percentiles, and the report says so. [inferred]
- The daemon restart inside `history_roundtrip` uses the profile's own daemon, never the desktop's service. [inferred]
- The pack never writes outside its evidence run directory and the scenario profiles; leaked processes fail the pack by name (QA-5). [paraphrase]

## Acceptance Criteria

- **R1:** `dettivo-qa pack dictation` runs every step in order in one evidence run, and `dictation-pack.json` plus `dictation-pack.md` name each step with outcome, duration, evidence path and reason; unit tests cover the pack's ordering, `--continue`, the `not_run` marking after a hard failure, and the report rendering from a fixture result. Errors: an unknown pack name exits 2 naming the packs. [inferred]
- **R2:** `history_roundtrip` passes on this machine and in CI: the dictated item is found by `transcripts.search`, survives a daemon restart, and `dictation.reinsert_last` inserts its text again through the mock target (the read-back matches). Errors: a missing item after the restart fails the step naming the id. [paraphrase]
- **R3:** `first_insert_timing` produces p50 and p95 release-to-inserted times from ten warm runs with the cold run excluded, split into capture, transcribe and insert from the new `timings` block; the report compares p50 with the NFR-1 target for the machine's tier and records, never gates, on CI. Errors: fewer than ten completed runs fail the step with the count. [paraphrase]
- **R4:** The insertion matrix rows are aggregated into `insertion_reliability` with skipped targets excluded and listed; on this Hyprland desktop the terminals, the Qt target and Chromium are attempted, and the figure is reported against the NFR-3 target. Errors: a target that failed is listed with its backend and reason. [paraphrase]
- **R5:** The pack runs in the CI drive job after the individual drives, uploads its report with the evidence, fails the job on a hard failure and passes with the allowed skips (Hyprland, Chromium, GPU); the Whisper WER fixture is a pack step on CPU in CI and on Vulkan here, reported with its threshold. Errors: as stated. [paraphrase]
- **R6:** `docs/qa.md` describes the pack, its report and the allowed skips; `docs/RELEASING.md` starts with the pack as the first named gate step and the rule that the blocker list must name every external blocker; the `timings` delta is registered with fixtures; an ADR records the pack model. Errors: as stated. [paraphrase]

## Boundaries

- No soak runs, benchmarks calibration or the CPU VM (S-16 and the soak spec). [paraphrase]
- No new scenarios beyond `history_roundtrip` and `first_insert_timing`; the pack composes what exists. [inferred]
- No release gate script beyond its first step; the release spec completes it. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- "Hold F9 in ghostty and text lands" is the phase 1 milestone; a pack that proves it on this machine and in CI, with the first NFR numbers, is what lets the build move to phase 2 with evidence rather than a feeling. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** the pack is the proof of the first complete workflow, with its numbers. [strategy:Complete speech workflows, proven by drives]
- **Local engines and performance:** the first time-to-insert measurement starts the calibration the strategy's key metric needs. [strategy:Local engines and performance]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
