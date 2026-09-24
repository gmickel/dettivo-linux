# GPU tiers, the doctor report and the benchmark suite

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-16 row: "`gpu-tiers-doctor-benchmarks` | phase 2 | depends on S-05, S-13, S-33 | FR-D5, FR-K3, NFR-1, 2, 4, 5, 6 calibration; QA-11 | `dettivo doctor --json` snapshot; benchmark report checked in"
> masterplan FR-D5: "`dettivo doctor` prints a human and `--json` report covering compositor, session type, portals, insertion backends, audio devices, GPU acceleration per engine, engine process state, models, LLM readiness, socket and service state."
> masterplan FR-K3: "The default build ships every ggml engine with Vulkan and CPU backends; runtime detection falls back to CPU when no Vulkan device exists."
> masterplan QA-11: "Benchmarks for NFR-1, 2, 4, 5 run from the same fixtures through engine CLI modes on Thor and the CPU VM and publish a checked-in report per release."
> masterplan NFR table: "NFR-1 time to first insert, warm engine, GPU tier, p50 ≤ 1.0 s ... NFR-2 CPU-only tier p50 ≤ 2.5 s ... NFR-4 meeting throughput GPU ≥ 10× realtime; diarization ≥ 4× ... NFR-5 CPU ≥ 2× realtime ... NFR-6 idle footprint: daemon idle RSS ≤ 60 MB with no engine running; engines unload after the idle timeout ... Targets are initial and are calibrated in the benchmark spec (S-16)."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

The numbers the product promises become a checked-in report. `dettivo-qa bench` runs the NFR-1, 2, 4 and 5 measurements from the same fixtures through the three engines' CLI modes (Whisper, Parakeet, the LLM) and the daemon's dictation path, on this GPU desktop and on a CPU-only run, records the idle footprint of NFR-6, and writes `docs/reports/benchmarks/<date>-<host>.json` with the machine's tier and every number beside its target. `dettivo doctor --json` becomes the FR-D5 snapshot: compositor, session, portals, insertion backends, audio devices, GPU acceleration per engine with the reason, engine process state, models, LLM readiness, socket and service state, and the tier the benchmarks use. The initial NFR targets get calibrated from the first report and recorded in an ADR. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Tier detection (`crates/dettivo-speech/src/tier.rs`): `gpu` when a Vulkan device enumerates and the selected engines report the Vulkan backend, `cpu` otherwise; exposed in `system.capabilities.platform.tier` and the doctor; `DETTIVO_FORCE_CPU=1` forces `cpu` for the CPU run on a GPU machine. [inferred]
- Doctor: `crates/dettivo-cli/src/doctor.rs` gains the missing FR-D5 rows in one stable JSON shape (`compositor`, `session_type`, `portals` with the interfaces found, `insertion` backends, `audio` devices with the default, `engines` with backend, reason, process state and memory, `models` readiness per provider, `llm` readiness and provider order, `socket`, `service` units, `rest`, `mcp`, `tier`); the JSON is fixture-snapshotted (machine-specific values levelled by shape like the replay does) and the human report groups the same rows. [paraphrase]
- `dettivo-qa bench` (`crates/dettivo-qa/src/bench/`): steps `first_insert` (the dictation pack's timing scenario reused for ten warm runs of the 11 s fixture per selected engine, p50 and p95 against NFR-1 or NFR-2 by tier), `stt_throughput` (the jfk clip repeated into a five-minute WAV through each STT engine's CLI mode, realtime factor against NFR-4 GPU or NFR-5 CPU), `llm_rewrite` (the Polish goldens' inputs through `dettivo-engine-llm --prompt`, tokens per second and latency p50, recorded), `idle_footprint` (daemon RSS after five idle minutes with engines unloaded, against NFR-6, plus the engines' unload observed through `speech.engines`), `startup` (socket ready after activation against NFR-8, app first frame from the app drive when a display exists); each step writes its numbers, the fixture hashes, the model ids, the backend and the reason, the host (CPU model, GPU name from the Vulkan enumeration, memory) and the git sha; `--cpu` forces the CPU tier; `--quick` runs three iterations for CI. Diarization throughput (NFR-4's second half) is a placeholder step reporting `not_available` until the diarization spec lands. [inferred]
- Report: `docs/reports/benchmarks/<YYYY-MM-DD>-<host>-<tier>.json` plus a rendered `docs/reports/benchmarks/README.md` table (latest per host and tier, target, met or not); `just bench` writes and updates them; CI runs `bench --quick --cpu` in the unit job and uploads the JSON as an artifact without checking it in. [paraphrase]
- Calibration: after the first report on this desktop (GPU) and the CPU run, the NFR targets are compared with the measurements and recorded in an ADR with the initial versus calibrated values; the pack's NFR lines read the calibrated targets from one place (`crates/dettivo-qa/src/nfr.rs`). [paraphrase]
- Release: `docs/RELEASING.md` gains the benchmark step (run `just bench` on the desktop, commit the report). [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo doctor --json` shape documented in `docs/daemon.md` with a fixture snapshot; `system.capabilities.platform.tier` and `engines[].memory_bytes` (Linux additions in the delta register). [inferred]
- `dettivo-qa bench [--cpu] [--quick] [--step <name>] [--out <dir>] [--json]`, `just bench`, `bench-quick`. [inferred]
- The report schema documented in `docs/qa.md` (versioned, `schema_version: 1`). [inferred]

## Edge Cases & Constraints

- No Vulkan device: the GPU steps are recorded as `not_available` with the reason, the CPU steps run. [paraphrase]
- A model missing: the step is skipped naming the download; never a failure of the suite. [inferred]
- Thermal or load noise: each step records the iteration count and the spread (p50, p95, min, max); the report never averages away a bad run. [inferred]
- CI runners: `--quick --cpu` only, numbers recorded, never gated. [paraphrase]

## Acceptance Criteria

- **R1:** `dettivo doctor --json` carries every FR-D5 row in the documented shape and matches its fixture by shape; the human report groups the same rows; the tier is reported. Errors: an unreachable daemon still prints the machine rows and exits 1. [paraphrase]
- **R2:** `dettivo-qa bench` runs every step on this desktop (GPU tier) and with `--cpu`, writes the report with numbers, spread, fixtures, models, backends, host and sha, and renders the README table; `--quick --cpu` runs in CI and uploads the JSON. Errors: a missing model skips the step naming the download. [paraphrase]
- **R3:** The first two reports (this desktop GPU and CPU) are checked in, and an ADR records the calibrated NFR-1, 2, 4, 5, 6 and 8 targets against the initial ones; the dictation pack reads the calibrated targets. Errors: as stated. [paraphrase]
- **R4:** Idle footprint: the daemon's RSS after five idle minutes with engines unloaded is measured and reported against NFR-6, and the engines' unload is observed. Errors: an engine still alive after the idle timeout is reported by name. [paraphrase]
- **R5:** `system.capabilities.platform.tier` and the engines' memory are registered deltas with fixtures; `DETTIVO_FORCE_CPU=1` forces the CPU tier through the whole path. Errors: as stated. [paraphrase]
- **R6:** `docs/qa.md` documents the suite and the report schema, `docs/daemon.md` the doctor shape, `docs/RELEASING.md` the benchmark step; the recipes are mirrored. Errors: as stated. [paraphrase]

## Boundaries

- No diarization or meeting throughput beyond the placeholder step (S-25, S-28). [paraphrase]
- No CPU VM; the CPU run is `--cpu` on this desktop until the VM exists, said so in the report. [inferred]
- No packaging (S-31). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The strategy's key metrics are numbers per hardware tier; a checked-in report per release is what makes them claims rather than hopes. [paraphrase]

## Strategy Alignment

- **Local engines and performance:** the tiers, the doctor and the benchmark report are the measurement the track is judged by. [strategy:Local engines and performance]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
