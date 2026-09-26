# One-command diarization bench for hill-climbing speaker accuracy

## Goal & Context

We want transcripts where every remote line carries the right speaker, for Whisper meetings with the current engine and with Nemotron. Getting there means many small changes to the engine, the assignment rule and the two-track handling. Each change needs a trustworthy number in seconds, not a day of hand-run scripts.

fn-64 (PR #14) built the pieces: `scripts/qa/nemotron3/` (a Nemotron ONNX runner, eval and scoring scripts, a local/remote track scorer, a blind judge pack, the segment adapter and a port of the product rule), and the strict DER scorer `scripts/qa/diarization_score.py` (ADR 0058). This spec turns them into one bench that anyone runs with one command. It scores the product's own Rust code, so a gain on the bench is a gain in the product.

Research behind it: vault note "dettivo-linux -- Speaker attribution research (2026-09-26)".

## Approach

- **One command.** `just diar-bench` scores the current tree against cached inputs in well under a minute. `just diar-bench --full` recomputes the engine outputs. `just diar-bench --baseline <name>` prints deltas against a saved run.
- **Cached stages.** Audio, engine output (turns, plus frame probabilities where the engine has them), the product's Whisper segments, assignment, then scoring. Each stage is cached by a content key (audio hash, engine, model, parameters), so changing the assignment rule reruns only assignment and scoring.
- **The product's code.** Assignment runs through the Rust implementation in `crates/dettivo-meeting` (exposed through `dettivo-qa` or a small CLI), not a Python port. Engines run through the product engine binaries. Until fn-69 lands, Nemotron runs through the fn-64 ONNX runner, labelled as such.
- **Splits that prevent overfitting.** AMI dev is the tuning set. AMI test is held out and shown only with `--heldout`. Retained Dettivo meetings are listed in a local selection file, split English and German by the meeting's language field. Labelled meetings from fn-72 join automatically when present.
- **One headline number.** Word-weighted attribution error: the share of reference words whose line did not get the right speaker, counting an unlabelled line as wrong. It sits beside its parts: wrong and unlabelled separately, DER split into missed, false alarm and confusion, the local/remote proxy, short-turn error (turns of three words or fewer), speed, and bootstrap confidence intervals over files.
- **Output.** A table in the terminal, and a JSON scoreboard in the protected eval directory. `--report` writes an aggregate-only report under `docs/reports/benchmarks/` when asked.

## Quick commands

- `just build test lint`
- `just diar-bench`

## Acceptance

- **R1:** `just diar-bench` runs from a clean checkout with cached inputs present and prints the scoreboard in under 60 seconds on Thor. It covers the current engine and Nemotron on AMI dev and the local English and German meetings.
- **R2:** A change to the assignment code in `crates/dettivo-meeting` changes the bench's numbers without an engine or ASR rerun. The run shows which stages were cached and which were recomputed.
- **R3:** The headline number and its parts are defined in one place in the code, and documented. The bench reproduces the fn-64 report's current-engine figures on the same inputs, within rounding.
- **R4:** `--baseline` shows per-metric deltas with bootstrap 95% intervals over files. `--save <name>` stores a baseline.
- **R5:** AMI test figures appear only with `--heldout`. The docs say to tune on dev and report test.
- **R6:** First-time setup is one command, `just diar-bench-setup`. It fetches public AMI audio and references, the models and the Python environment, and writes the local selection file from retained meetings, while skipping any meeting that is recording or finalising.
- **R7:** Nothing confidential enters git: no audio, transcripts, names or meeting IDs. Caches and scoreboards live in the protected eval directory (mode 700), and a test asserts the report writer emits aggregates only.
- **R8:** `docs/qa.md` (or a linked page) explains the bench in a short "how to hill-climb" section: the loop, the metrics, the splits and how to add a variant.

## Boundaries

- No change to product behaviour. The bench measures; other specs change the product.
- Not a CI job and not a release gate (ADR 0066 spirit). It runs by hand on the desktop.
- No LLM correction pass.

## Decision Context

Gordon asked on 2026-09-26 for "a really easy way to test our progress and hill-climb efficiently", building on the fn-64 work. It comes first because fn-68 to fn-72 are all judged with it.
