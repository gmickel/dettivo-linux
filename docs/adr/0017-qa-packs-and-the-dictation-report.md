# 0017. Packs compose scenarios into one release-shaped report; the completion event carries the timing split

Status: Accepted 2026-09-04

## What this gives you

One command, `dettivo-qa pack dictation`, proves the whole dictation slice on the machine in front of you and writes a report that says what passed, what could not run and why, and the first numbers for time to first insert and insertion reliability. The same command is the phase 1 exit check on the development machine, the last step of the CI drive job, and the first named step of the release gate.

## Situation

Phase 1 ends with hotkeys, the pill, insertion, the history store and the dictation session each proven by its own scenario, and the masterplan's exit criteria ask for them together: a full dictation drive on the development machine and in CI, the Whisper WER fixture on CPU and Vulkan, and the first NFR-1 and NFR-3 measurements. The release gate (QA-9) is one script with named steps and JSON output whose "only external blockers remain" claim must name the blockers. The scenarios are written and lint-checked against one driver interface; a second way to run them would fork the rig. The daemon's `dictation.state` stream had timestamps per transition but no way to tell how much of the path after the key release was capture, transcription or insertion.

## Decision

A pack is an ordered list of steps in `crates/dettivo-qa/src/pack/`: each step is a scenario id (or the Whisper WER check, which is not a scenario because it drives the engine binary, not the product) with a pinned driver or the run's `--driver`, an expectation (`must pass`, or `skip allowed`), and the measurement it feeds. Scenarios are not changed; a pack composes them through the existing runner into one run directory, `qa-evidence/<run>/pack-<name>/`, and stops after a hard failure unless `--continue`, marking every later step `not_run`. A skip on a `skip allowed` step is a blocker the report names; a skip on a `must pass` step is a failure. The report is `<name>-pack.json` and `<name>-pack.md`: machine and tier, one row per step with outcome, duration, evidence directory and reason, the measurements, and the blocker list. Exit 0 means every step passed or skipped for an allowed reason, 1 a hard failure, 2 a preflight refusal.

The dictation pack's steps, in order: `hotkeys_hyprland`, `osd_dictation` on `atspi` and on `cua`, `insertion_matrix`, `never_into_self`, `history_roundtrip` (must pass), `first_insert_timing`, `whisper_wer` (must pass). The two new scenarios drive the socket and the Qt target only, so they run under Xvfb and on Hyprland alike.

The tier is `gpu` when the Whisper engine reports Vulkan, `cpu` otherwise, read from the WER step or the timing scenario rather than guessed from installed ICDs. NFR-1 has a target for the GPU tier only (p50 at or under 1000 ms); the CPU tier records its figure with no target until the soak and benchmark spec calibrates one. The NFR comparison is recorded in the report and never gates the exit code, on CI or on the development machine; the release spec decides what gates.

`dictation.state`'s completion transition gains an optional `timings` block (`capture_ms`, `transcribe_ms`, `insert_ms`), a Linux addition registered in the delta register, measured in the session worker around the drain of the take after the stop, the engine's `recognize` call and the inserter's call. The timing scenario measures the whole path on its own clock, from its stop request to the completion event's arrival, and uses the block to split it. The first run of eleven warms the engine and is excluded from the percentiles; p50 and p95 use nearest rank over the ten warm runs.

## Consequences

- The release gate starts with `just qa-pack dictation` and the rule that its blocker list must name every external blocker (`docs/RELEASING.md`); the flow-next QA pass reads the same JSON.
- CI runs the pack after the individual drives in the same Xvfb session and uploads the report with the evidence; there the hotkeys step, the terminals, Chromium and Vulkan are allowed skips, and the NFR figures are recorded, never gated.
- The pack costs its steps' time twice on CI (the individual drives still run first, so a failing scenario is named on its own before the pack), and the timing scenario adds about three minutes of fixture playback.
- Reliability is passes over attempted targets; a target the machine cannot reach is excluded and listed, so the figure never improves by skipping.
- The WER step runs the engine as built: `just build-whisper-vulkan` before the pack makes the development-machine figure a Vulkan one, and a workspace build without the feature makes it a CPU one; the report says which.
- A pack never writes outside its run directory and the scenarios' profiles; a leaked process fails its step by name through the runner's existing check.
