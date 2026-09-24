# QA pack: meetings end to end on the rig and with mock fixtures, the NFR-4 and NFR-5 throughput report and the CPU-only benchmark path

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-28 row: "`qa-pack-meetings-end-to-end` | phase 4 | depends on S-25 to S-27 | NFR-4, NFR-5 measurement | Full meeting drive on Thor; CPU VM benchmark"
> masterplan NFR-4: "Meeting throughput, GPU tier | ≥ 10× realtime transcription; diarization ≥ 4× realtime | Engine benchmark fixtures"
> masterplan NFR-5: "Meeting throughput, CPU tier | ≥ 2× realtime for `base.en` or Parakeet q8 | Engine benchmark fixtures"
> masterplan phase 4 exit criteria: "Virtual rig meeting scenario passes end to end including diarization and recovery; export goldens pass; meetings GUI drive pack green"
> masterplan QA-11: "Benchmarks for NFR-1, 2, 4, 5 run from the same fixtures through engine CLI modes on Thor and the CPU VM and publish a checked-in report per release."
> masterplan QA-6: "The virtual audio rig can inject microphone and system fixtures, hot-swap devices, and is exercised by the dictation and meeting packs."
> masterplan 9.5: "Scripted fixtures are generated at test time with a local TTS voice and asserted by required-token coverage rather than exact WER ... Two-speaker meeting fixtures use two TTS voices routed to the microphone and system sinks respectively so diarization has a ground truth." and "a GPU-workload proof that transcribes a long fixture and requires the engine process pid to appear in `nvidia-smi` or the equivalent AMD counter during the run."
> masterplan 9.10: "CPU-only VM (Arch, Hyprland or Sway, PipeWire dummy driver) | NFR-2, NFR-5, CI parity checks, packaging install test"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Phase 4 closes on one command. `dettivo-qa pack meetings` runs the meeting lane in order on the machine in front of you: two-source capture on the virtual rig with a device swap, a kill and recovery, the live windowed transcript, diarization against the turns golden, the export goldens, the three meetings GUI drives on both drivers, and a two-voice scripted fixture whose required tokens must all survive capture, transcription and diarization. The same pack runs under Xvfb in CI with `DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_SYSTEM_AUDIO` and `tiny.en`. It measures meeting throughput for NFR-4 and NFR-5 through the daemon's own finalisation path and the diarization engine's CLI mode, proves the GPU tier did its work on the GPU, records the CPU tier with `--cpu` here and on the CPU-only VM, and writes the figures into the checked-in benchmark report the GPU-tiers spec started. One report, `meetings-pack.json` with `meetings-pack.md`, is the phase 4 exit check and the third named step of the release gate. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Pack (`crates/dettivo-qa/src/pack/meetings.rs`, the ADR 0017 model): steps in order `meeting_capture` (the capture spec's two-source rig scenario with the mid-meeting sink unload and the gap marker), `meeting_recover` (its kill-and-recover scenario: SIGKILL the profile's daemon, restart, `meetings.recover`, the finalised transcript covers both takes), `meeting_live` (the transcription spec's rig scenario against the interleave golden), `diarization_der` (`dettivo-qa pipeline diarization` against `two-speakers.turns.json`, error rate under 0.20), `meeting_export_goldens` (`cargo test -p dettivo-storage meeting_export`, a binary step like `whisper_wer`), `meetings_seeded`, `meetings_live_gui` and `meetings_import_gui` on `atspi` then `cua`, a new `meeting_token_coverage` scenario, `meeting_throughput`, `diarization_throughput` and `gpu_workload_proof`. The rig steps and the three GUI drives are must-pass; the GPU proof is skip-allowed off a GPU machine. [inferred]
- Token fixture: `crates/dettivo-qa/fixtures/meetings/alpha.mic.wav` and `alpha.system.wav` (two local TTS voices reading the 9.5 script, rendered once and checked in with the script and `alpha.tokens.json`: `alice`, `launch`, `checklist`, `ben`, `pricing`, `friday`, `decision`, `ship`, with the source each token belongs to); the scenario plays them through the rig sinks here and the mock variables in CI, stops, and asserts every token in the finalised transcript on its source, two speakers after diarization, and the `md` export carrying both names. [paraphrase]
- Throughput: `meeting_throughput` builds a five-minute two-source fixture (the jfk clip repeated on the system track, the alpha microphone track once, silence between) and runs it through `transcripts.import { target_kind: "meeting" }` so the chunked pipeline, the merger and the store are all timed; the realtime factor is audio duration over the job's wall time from `job.progress` `decoding` to `done`, with the engine, model and backend from `speech.engines`. `diarization_throughput` runs `dettivo-engine-diarize --wav --model --json` over the same system track for its factor. Targets come from `crates/dettivo-qa/src/nfr.rs` (the calibrated NFR-4 and NFR-5 values by tier); the tier is the daemon's `platform.tier`. Under CI the figures are recorded, never gated, as the dictation pack does. [inferred]
- CPU path: `--cpu` sets `DETTIVO_FORCE_CPU=1` for the daemon and the engines and selects `[speech] meeting_model` from `--model` (default `base.en`, the NFR-5 model), so the same command on this desktop records the CPU tier and on the CPU-only VM records the VM; the report names the host and the tier so the two rows never merge. `gpu_workload_proof` samples `nvidia-smi --query-compute-apps=pid --format=csv` (or `/sys/class/drm/*/device/gpu_busy_percent` for AMD) every 500 ms during `meeting_throughput` and requires the whisper or parakeet engine pid to appear. Harness load is measured with the driver idle before the throughput steps and written beside the figures (9.4 item 10). [paraphrase]
- Report: `qa-evidence/<run>/pack-meetings/meetings-pack.json` and `.md` with the ADR 0017 shape plus `measurements: { meeting_rtf, meeting_rtf_target, diarization_rtf, diarization_rtf_target, tier, engine, model, backend, audio_ms, wall_ms, fixture_sha, harness_cpu_pct, token_coverage }`; `--record` appends a `meetings` block to `docs/reports/benchmarks/<date>-<host>-<tier>.json` and re-renders the README table through `dettivo-qa bench`'s writer, which is how the report is checked in per release. [inferred]
- Profiles: every step runs in its own scenario root with the model directory linked in and nothing from the caller's home; the rig sinks are created and unloaded per step by `scripts/qa/audio-rig.sh` and a leaked sink fails the step by name. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo-qa pack meetings [--driver atspi|cua] [--cpu] [--model <id>] [--record] [--continue] [--json] [--out <dir>]`; exit codes as ADR 0017. `dettivo-qa bench` gains `meeting_throughput` and `diarization_throughput` steps that share the pack's code. [inferred]
- `just qa-pack meetings` and `just qa-pack-meetings-cpu` (the `--cpu --record` form documented for the VM); the CI drive job runs the pack after the GUI pack with the mock variables and the cached `tiny.en` and diarization model set from `scripts/models/fetch-test-model.sh`. [inferred]
- No new daemon methods; the fixtures, the tokens file and the report schema are documented in `docs/qa.md`. [inferred]

## Edge Cases & Constraints

- No PipeWire (a container): the rig steps run with the mock variables and the report says `mock`; the device-swap assertion uses the mock's take switch. [paraphrase]
- The diarization model set absent: the step fails naming the download command; CI caches it, so absence is a CI defect, not a skip. [inferred]
- An engine slower than realtime during `meeting_live`: skipped windows are journaled and the finalised transcript must still match the golden. [paraphrase]
- A token missing from the finalised transcript: the step fails naming the token and its source, with the transcript in the evidence. [inferred]
- `gpu_workload_proof` on a machine without `nvidia-smi` or the AMD counter: skipped with the reason; on this desktop it must pass. [paraphrase]
- Harness CPU above 10 percent during the throughput step: the figure is recorded with a warning and the report says the load was not the product's. [inferred]

## Acceptance Criteria

- **R1:** `dettivo-qa pack meetings` runs every step in order in one evidence run and writes `meetings-pack.json` plus `meetings-pack.md` with the measurements block and the blockers; unit tests cover the step order, `--cpu`, `--record`'s report merge and the token assertion. Errors: an unknown model id exits 2 naming the catalogue ids. [inferred]
- **R2:** The rig steps pass here on the virtual audio rig and in CI with the mock fixtures: two-source capture with the gap marker after the swap, recovery after SIGKILL with the transcript covering both takes, the live transcript matching the interleave golden, and every alpha token on its source with two named speakers in the `md` export. Errors: a missing token or a missing gap marker fails naming it. [paraphrase]
- **R3:** `diarization_der` passes under 0.20 with two speakers, `meeting_export_goldens` renders the five formats byte-equal, and `meetings_seeded`, `meetings_live_gui` and `meetings_import_gui` pass on both drivers under Xvfb and here. Errors: as stated. [paraphrase]
- **R4:** On this desktop the GPU tier's `meeting_throughput` and `diarization_throughput` record realtime factors against the NFR-4 targets with the engine, model and backend named, `gpu_workload_proof` sees the engine pid on the GPU, and `--record` writes the block into the checked-in benchmark report and the README table. Errors: a factor below target is recorded with the target beside it, never silently passed. [paraphrase]
- **R5:** `--cpu` on this desktop and the same command on the CPU-only VM record the CPU tier with `base.en` against NFR-5 as two rows in the checked-in report, and CI records the mock-tier figures without gating; the pack exits 0 with the GPU proof as the only allowed skip on the VM. Errors: a run whose engine reports a GPU backend under `--cpu` fails naming the engine. [paraphrase]
- **R6:** `docs/qa.md` gains the meetings pack section (steps, fixtures, the token rule, the throughput method, the CPU path, the report schema), `docs/RELEASING.md` adds the pack as the third named gate step with the benchmark rows it must have, the CI workflow runs it, and an ADR records the finalisation-path throughput measurement and the GPU proof. Errors: `just docs` fails on an unindexed ADR. [paraphrase]

## Boundaries

- No new meeting behaviour; a gap a step exposes is fixed in the owning spec. [inferred]
- No soak runs, no NFR recalibration (the ADR of the GPU-tiers spec owns the targets), no live diarization. [paraphrase]
- No GUI pack steps beyond the three meetings drives (S-22) and no visual re-approval (S-36). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Long-audio throughput is a key metric of the strategy and meetings are half the product promise; a pack that runs the whole lane on real audio paths, proves the GPU did the work and checks the numbers in per release is what makes the promise measurable. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** capture, recovery, live transcript, diarization, export and the three screens in one run with one report. [strategy:Complete speech workflows, proven by drives]
- **Local engines and performance:** NFR-4 and NFR-5 measured through the product's own path per tier and checked in. [strategy:Local engines and performance]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
